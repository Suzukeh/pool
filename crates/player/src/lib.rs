//! 再生クロック：フレーム⇔秒変換とループ送り（純粋関数）。

/// fps = fps_num / fps_den。
#[derive(Debug, Clone, Copy)]
pub struct PlaybackClock {
    pub fps_num: u32,
    pub fps_den: u32,
}

impl PlaybackClock {
    pub fn new(fps_num: u32, fps_den: u32) -> Self {
        Self {
            fps_num: fps_num.max(1),
            fps_den: fps_den.max(1),
        }
    }

    pub fn fps(&self) -> f64 {
        self.fps_num as f64 / self.fps_den as f64
    }

    pub fn frame_to_seconds(&self, frame: i64) -> f64 {
        frame as f64 / self.fps()
    }

    pub fn seconds_to_frame(&self, seconds: f64) -> i64 {
        (seconds * self.fps()).round() as i64
    }

    /// 次フレーム。`duration`（総フレーム、>0）に達したら 0 に巻き戻す。
    pub fn next_frame(&self, current: i64, duration: i64) -> i64 {
        let next = current + 1;
        if next >= duration.max(1) { 0 } else { next }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_both_ways() {
        let c = PlaybackClock::new(30, 1);
        assert_eq!(c.seconds_to_frame(10.0), 300);
        assert!((c.frame_to_seconds(300) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn loops_at_duration() {
        let c = PlaybackClock::new(30, 1);
        assert_eq!(c.next_frame(299, 300), 0);
        assert_eq!(c.next_frame(10, 300), 11);
    }
}
