//! A VNC client in the backend (vnc-rs), for AI agents: keeps a copy of a VM's screen and
//! sends mouse and keyboard input, over the same SSH-forwarded connection the GUI uses.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::client::Msg;
use tokio::sync::Mutex;
use vnc::{ClientKeyEvent, ClientMouseEvent, PixelFormat, VncConnector, VncEncoding, VncEvent, X11Event};

use crate::ssh::Conn;

struct Frame {
    width: usize,
    height: usize,
    /// RGBA, row by row.
    pixels: Vec<u8>,
    last_update: Instant,
    /// Rectangles received so far; 0 means no picture yet.
    updates: u64,
}

impl Frame {
    fn resize(&mut self, w: usize, h: usize) {
        self.width = w;
        self.height = h;
        self.pixels = vec![0; w * h * 4];
    }

    fn put(&mut self, x: usize, y: usize, w: usize, h: usize, data: &[u8]) {
        for row in 0..h {
            let (dy, sx) = (y + row, row * w * 4);
            if dy >= self.height || x >= self.width {
                continue;
            }
            let n = w.min(self.width - x) * 4;
            let d = (dy * self.width + x) * 4;
            if let Some(src) = data.get(sx..sx + n) {
                self.pixels[d..d + n].copy_from_slice(src);
            }
        }
        self.last_update = Instant::now();
        self.updates += 1;
    }

    fn copy(&mut self, dx: usize, dy: usize, sx: usize, sy: usize, w: usize, h: usize) {
        let mut tmp = vec![0u8; w * h * 4];
        for row in 0..h {
            let s = ((sy + row) * self.width + sx) * 4;
            if let Some(src) = self.pixels.get(s..s + w * 4) {
                tmp[row * w * 4..(row + 1) * w * 4].copy_from_slice(src);
            }
        }
        self.put(dx, dy, w, h, &tmp);
    }
}

pub struct Screen {
    client: Arc<vnc::VncClient>,
    frame: Arc<Mutex<Frame>>,
    pointer: Mutex<(u16, u16, u8)>,
    pump: tokio::task::JoinHandle<()>,
    _conn: Arc<Conn>,
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.pump.abort();
    }
}

pub struct Shot {
    pub png: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub screen_width: usize,
    pub screen_height: usize,
}

impl Screen {
    pub async fn connect(conn: Arc<Conn>, channel: Channel<Msg>, password: String) -> Result<Self> {
        let stream = channel.into_stream();
        let client = VncConnector::new(stream)
            .set_auth_method(async move { if password.is_empty() { Err(vnc::VncError::NoPassword) } else { Ok(password) } })
            .add_encoding(VncEncoding::Zrle)
            .add_encoding(VncEncoding::CopyRect)
            .add_encoding(VncEncoding::Raw)
            .add_encoding(VncEncoding::DesktopSizePseudo)
            .allow_shared(true)
            .set_pixel_format(PixelFormat::rgba())
            .build()
            .map_err(|e| anyhow!("VNC: {e}"))?
            .try_start()
            .await
            .map_err(|e| anyhow!("VNC handshake: {e} (a VNC password can be set in the VM's settings)"))?
            .finish()
            .map_err(|e| anyhow!("VNC: {e}"))?;
        let client = Arc::new(client);
        let frame = Arc::new(Mutex::new(Frame { width: 0, height: 0, pixels: Vec::new(), last_update: Instant::now(), updates: 0 }));
        let pump = tokio::spawn(pump(client.clone(), frame.clone()));
        let screen = Self { client, frame, pointer: Mutex::new((0, 0, 0)), pump, _conn: conn };
        // Wait for the first full picture.
        let started = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let f = screen.frame.lock().await;
            if f.updates > 0 && f.last_update.elapsed() > Duration::from_millis(400) {
                break;
            }
            if started.elapsed() > Duration::from_secs(20) {
                bail!("the VNC server sent no picture within 20 s");
            }
        }
        Ok(screen)
    }

    pub fn is_alive(&self) -> bool {
        !self.pump.is_finished()
    }

    pub async fn size(&self) -> (usize, usize) {
        let f = self.frame.lock().await;
        (f.width, f.height)
    }

    /// Waits until the picture has been still for a moment (or 2 s passed), then encodes it.
    pub async fn screenshot(&self, max_side: usize) -> Result<Shot> {
        let _ = self.client.input(X11Event::Refresh).await;
        let start = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(120)).await;
            let quiet = self.frame.lock().await.last_update.elapsed() > Duration::from_millis(250);
            if quiet || start.elapsed() > Duration::from_secs(2) {
                break;
            }
        }
        let f = self.frame.lock().await;
        if f.width == 0 {
            bail!("no picture yet");
        }
        let scale = (max_side.max(64) as f64 / f.width.max(f.height) as f64).min(1.0);
        let (w, h) = (((f.width as f64) * scale).round() as usize, ((f.height as f64) * scale).round() as usize);
        let rgb = downscale(&f.pixels, f.width, f.height, w.max(1), h.max(1));
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, w.max(1) as u32, h.max(1) as u32);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().context("PNG")?;
            wr.write_image_data(&rgb).context("PNG")?;
        }
        Ok(Shot { png, width: w.max(1), height: h.max(1), screen_width: f.width, screen_height: f.height })
    }

    async fn pointer(&self, x: u16, y: u16, buttons: u8) -> Result<()> {
        *self.pointer.lock().await = (x, y, buttons);
        self.client
            .input(X11Event::PointerEvent(ClientMouseEvent { position_x: x, position_y: y, bottons: buttons }))
            .await
            .map_err(|e| anyhow!("VNC: {e}"))
    }

    async fn clamp(&self, x: f64, y: f64) -> (u16, u16) {
        let (w, h) = self.size().await;
        (x.round().clamp(0.0, (w.max(1) - 1) as f64) as u16, y.round().clamp(0.0, (h.max(1) - 1) as f64) as u16)
    }

    /// Moves the pointer; coordinates are in the screen's own pixels.
    pub async fn move_to(&self, x: f64, y: f64) -> Result<()> {
        let (x, y) = self.clamp(x, y).await;
        self.pointer(x, y, 0).await
    }

    /// button: 1 left, 2 middle, 3 right.
    pub async fn click(&self, x: f64, y: f64, button: u8, count: u8) -> Result<()> {
        let (x, y) = self.clamp(x, y).await;
        let mask = match button {
            2 => 2,
            3 => 4,
            _ => 1,
        };
        self.pointer(x, y, 0).await?;
        for _ in 0..count.max(1) {
            self.pointer(x, y, mask).await?;
            tokio::time::sleep(Duration::from_millis(40)).await;
            self.pointer(x, y, 0).await?;
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
        Ok(())
    }

    /// Positive `dy` scrolls down, negative up, in wheel steps.
    pub async fn scroll(&self, x: f64, y: f64, dy: i32) -> Result<()> {
        let (x, y) = self.clamp(x, y).await;
        let mask = if dy < 0 { 8 } else { 16 };
        for _ in 0..dy.unsigned_abs().min(50) {
            self.pointer(x, y, mask).await?;
            self.pointer(x, y, 0).await?;
        }
        Ok(())
    }

    async fn key(&self, keysym: u32, down: bool) -> Result<()> {
        self.client
            .input(X11Event::KeyEvent(ClientKeyEvent { keycode: keysym, down }))
            .await
            .map_err(|e| anyhow!("VNC: {e}"))
    }

    pub async fn type_text(&self, text: &str) -> Result<()> {
        for c in text.chars() {
            let sym = match c {
                '\n' => 0xff0d,
                '\t' => 0xff09,
                c => char_keysym(c),
            };
            self.key(sym, true).await?;
            self.key(sym, false).await?;
            tokio::time::sleep(Duration::from_millis(8)).await;
        }
        Ok(())
    }

    /// Presses a combination like "ctrl+alt+delete", "win+r" or "enter".
    pub async fn press(&self, combo: &str) -> Result<()> {
        let mut syms = Vec::new();
        for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            syms.push(key_keysym(part).with_context(|| format!("unknown key '{part}'"))?);
        }
        if syms.is_empty() {
            bail!("no key given");
        }
        for s in &syms {
            self.key(*s, true).await?;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
        for s in syms.iter().rev() {
            self.key(*s, false).await?;
        }
        Ok(())
    }
}

async fn pump(client: Arc<vnc::VncClient>, frame: Arc<Mutex<Frame>>) {
    let mut last_refresh = Instant::now();
    let _ = client.input(X11Event::FullRefresh).await;
    loop {
        let mut got = false;
        loop {
            match client.poll_event().await {
                Ok(Some(ev)) => {
                    got = true;
                    let mut f = frame.lock().await;
                    match ev {
                        VncEvent::SetResolution(s) => f.resize(s.width as usize, s.height as usize),
                        VncEvent::RawImage(r, data) => {
                            f.put(r.x as usize, r.y as usize, r.width as usize, r.height as usize, &data)
                        }
                        VncEvent::Copy(d, s) => {
                            f.copy(d.x as usize, d.y as usize, s.x as usize, s.y as usize, d.width as usize, d.height as usize)
                        }
                        VncEvent::Error(e) => {
                            log::info!("VNC: {e}");
                            return;
                        }
                        _ => {}
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::info!("VNC: {e}");
                    return;
                }
            }
        }
        if last_refresh.elapsed() > Duration::from_millis(150) {
            if client.input(X11Event::Refresh).await.is_err() {
                return;
            }
            last_refresh = Instant::now();
        }
        if !got {
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }
}

/// Box-filter downscale from RGBA to RGB.
fn downscale(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 3];
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1).min(sh));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1).min(sw));
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * sw + xx) * 4;
                    r += src[i] as u32;
                    g += src[i + 1] as u32;
                    b += src[i + 2] as u32;
                    n += 1;
                }
            }
            let o = (y * dw + x) * 3;
            let n = n.max(1);
            out[o] = (r / n) as u8;
            out[o + 1] = (g / n) as u8;
            out[o + 2] = (b / n) as u8;
        }
    }
    out
}

fn char_keysym(c: char) -> u32 {
    let cp = c as u32;
    if (0x20..=0x7e).contains(&cp) || (0xa0..=0xff).contains(&cp) { cp } else { 0x0100_0000 + cp }
}

fn key_keysym(name: &str) -> Option<u32> {
    let n = name.to_ascii_lowercase();
    Some(match n.as_str() {
        "ctrl" | "control" | "strg" => 0xffe3,
        "shift" => 0xffe1,
        "alt" => 0xffe9,
        "altgr" => 0xfe03,
        "win" | "super" | "meta" | "cmd" | "windows" => 0xffeb,
        "enter" | "return" => 0xff0d,
        "tab" => 0xff09,
        "esc" | "escape" => 0xff1b,
        "backspace" => 0xff08,
        "delete" | "del" | "entf" => 0xffff,
        "insert" | "ins" => 0xff63,
        "home" | "pos1" => 0xff50,
        "end" => 0xff57,
        "pageup" | "pgup" => 0xff55,
        "pagedown" | "pgdn" => 0xff56,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "space" => 0x20,
        "printscreen" | "print" => 0xff61,
        "capslock" => 0xffe5,
        f if f.len() >= 2 && f.starts_with('f') && f[1..].parse::<u32>().is_ok() => {
            let k: u32 = f[1..].parse().ok()?;
            if !(1..=24).contains(&k) {
                return None;
            }
            0xffbe + k - 1
        }
        s if s.chars().count() == 1 => char_keysym(name.chars().next()?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(key_keysym("ctrl"), Some(0xffe3));
        assert_eq!(key_keysym("F5"), Some(0xffc2));
        assert_eq!(key_keysym("r"), Some('r' as u32));
        assert_eq!(key_keysym("nope"), None);
        assert_eq!(char_keysym('ä'), 0xe4);
        assert_eq!(char_keysym('€'), 0x0100_20ac);
    }

    #[test]
    fn scales_down() {
        let src = vec![255u8; 4 * 4 * 4];
        let out = downscale(&src, 4, 4, 2, 2);
        assert_eq!(out.len(), 12);
        assert!(out.iter().all(|&v| v == 255));
    }
}
