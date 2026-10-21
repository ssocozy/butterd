//! Event wakeups are hints; authoritative state is always read again before tuning.
use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, sync::mpsc::{self, Receiver, SyncSender}, thread::{self, JoinHandle}, time::Duration};

pub fn relevant(line: &str) -> bool {
    ["am_on_resume_called", "am_resume_activity", "screen_toggled", "power_screen_state", "battery_status", "battery_level", "butterWake"]
        .iter().any(|tag| line.split_once(*tag).is_some_and(|(_, tail)| tail.starts_with('(') || tail.starts_with(':') || tail.starts_with(' ')))
}
// Bound line memory even if a ROM emits an oversized record. Event payloads are not retained.
fn read_events(reader: &mut impl BufRead, sender: SyncSender<()>) {
    let mut record = Vec::with_capacity(512);
    while let Ok(bytes) = reader.fill_buf() {
        if bytes.is_empty() { break; }
        let end = bytes.iter().position(|byte| *byte == b'\n');
        let count = end.map_or(bytes.len(), |index| index + 1);
        let remaining = 4096usize.saturating_sub(record.len());
        record.extend_from_slice(&bytes[..count.min(remaining)]);
        reader.consume(count);
        if end.is_some() {
            if relevant(&String::from_utf8_lossy(&record)) { let _ = sender.try_send(()); }
            record.clear();
        }
    }
}
pub struct WakeEvents { child: Option<Child>, reader: Option<JoinHandle<()>>, receiver: Option<Receiver<()>>, pub last_wake: &'static str }
impl Default for WakeEvents { fn default() -> Self { Self::new() } }
impl WakeEvents {
    pub fn new() -> Self {
        let mut source = Self { child:None, reader:None, receiver:None, last_wake:"startup" };
        let process = Command::new("/system/bin/logcat").args(["-b", "events", "-b", "main", "-v", "brief", "-T", "1", "am_on_resume_called:I", "am_resume_activity:I", "screen_toggled:I", "power_screen_state:I", "battery_status:I", "battery_level:I", "butterWake:I", "*:S"])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = process {
            if let Some(output) = child.stdout.take() {
                let (sender, receiver) = mpsc::sync_channel(1);
                source.reader = Some(thread::spawn(move || read_events(&mut BufReader::new(output), sender)));
                source.receiver = Some(receiver);
            }
            source.child = Some(child);
        }
        source
    }
    pub fn available(&mut self) -> bool { self.child.as_mut().is_some_and(|child| matches!(child.try_wait(), Ok(None))) }
    pub fn wait(&mut self, timeout: Duration) {
        let wake = self.receiver.as_ref().map(|receiver| receiver.recv_timeout(timeout));
        match wake {
            Some(Ok(())) => {
                self.last_wake = "android-event";
                // Coalesce lifecycle bursts; never issue a write directly from a log record.
                thread::sleep(Duration::from_millis(150));
                if let Some(receiver) = &self.receiver { while receiver.try_recv().is_ok() {} }
            }
            Some(Err(mpsc::RecvTimeoutError::Timeout)) => self.last_wake = "fallback-or-deadline",
            _ => { thread::sleep(timeout); *self = Self::new(); self.last_wake = "fallback-reconnect"; },
        }
    }
}
impl Drop for WakeEvents {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child { let _ = child.kill(); let _ = child.wait(); }
        if let Some(reader) = self.reader.take() { let _ = reader.join(); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_are_bounded_and_coalesced() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut data = vec![b'x'; 20000]; data.extend_from_slice(b"\nI/am_on_resume_called(123): app\nI/battery_status(123): charging\n");
        read_events(&mut std::io::Cursor::new(data), sender);
        assert!(receiver.try_recv().is_ok()); assert!(receiver.try_recv().is_err());
        assert!(!relevant("I/Other(1): regular log"));
    }
}
