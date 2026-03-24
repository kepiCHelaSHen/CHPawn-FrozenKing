use shakmaty::{Chess, Color, Position};
use shakmaty_syzygy::{AmbiguousWdl, Dtz, MaybeRounded, Tablebase, Wdl};
use std::path::Path;

const TB_WIN_SCORE: i32 = 500_000;

pub struct TablebaseProber {
    tb: Tablebase<Chess>,
    available: bool,
}

impl TablebaseProber {
    pub fn new(path: &str) -> Self {
        let mut tb = Tablebase::new();
        let available = if Path::new(path).exists() {
            match tb.add_directory(path) {
                Ok(_) => true,
                Err(_) => false,
            }
        } else {
            false
        };
        TablebaseProber { tb, available }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Probe WDL and return score from WHITE's perspective.
    /// Uses DTZ to prefer faster wins / slower losses.
    pub fn probe_wdl(&self, pos: &Chess) -> Option<i32> {
        if !self.available {
            return None;
        }
        let wdl = self.tb.probe_wdl(pos).ok()?;
        let dtz = self
            .tb
            .probe_dtz(pos)
            .ok()
            .map(|d| dtz_abs(d))
            .unwrap_or(0);
        let side_score = match wdl {
            AmbiguousWdl::Win => TB_WIN_SCORE - dtz,
            AmbiguousWdl::CursedWin | AmbiguousWdl::MaybeWin => TB_WIN_SCORE / 2 - dtz,
            AmbiguousWdl::Draw => 0,
            AmbiguousWdl::BlessedLoss | AmbiguousWdl::MaybeLoss => -(TB_WIN_SCORE / 2) + dtz,
            AmbiguousWdl::Loss => -TB_WIN_SCORE + dtz,
        };
        // Convert from side-to-move perspective to WHITE's perspective
        if pos.turn() == Color::White {
            Some(side_score)
        } else {
            Some(-side_score)
        }
    }

    /// Probe WDL only, return simplified Wdl from side-to-move perspective.
    pub fn probe_wdl_raw(&self, pos: &Chess) -> Option<Wdl> {
        if !self.available {
            return None;
        }
        let awdl = self.tb.probe_wdl(pos).ok()?;
        Some(ambiguous_to_wdl(awdl))
    }

    /// Probe DTZ, return raw i32 value.
    pub fn probe_dtz_raw(&self, pos: &Chess) -> Option<i32> {
        if !self.available {
            return None;
        }
        let mr = self.tb.probe_dtz(pos).ok()?;
        Some(dtz_value(mr))
    }
}

fn dtz_abs(mr: MaybeRounded<Dtz>) -> i32 {
    let d = mr.ignore_rounding();
    (i32::from(d)).unsigned_abs() as i32
}

fn dtz_value(mr: MaybeRounded<Dtz>) -> i32 {
    let d = mr.ignore_rounding();
    i32::from(d)
}

fn ambiguous_to_wdl(awdl: AmbiguousWdl) -> Wdl {
    match awdl {
        AmbiguousWdl::Win | AmbiguousWdl::MaybeWin => Wdl::Win,
        AmbiguousWdl::CursedWin => Wdl::CursedWin,
        AmbiguousWdl::Draw => Wdl::Draw,
        AmbiguousWdl::BlessedLoss => Wdl::BlessedLoss,
        AmbiguousWdl::Loss | AmbiguousWdl::MaybeLoss => Wdl::Loss,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::fen::Fen;
    use shakmaty::CastlingMode;

    fn pos_from_fen(fen: &str) -> Chess {
        let f: Fen = fen.parse().unwrap();
        f.into_position(CastlingMode::Standard).unwrap()
    }

    fn get_tb() -> TablebaseProber {
        TablebaseProber::new("syzygy")
    }

    #[test]
    fn kqvk_white_winning() {
        let tb = get_tb();
        if !tb.is_available() {
            return;
        }
        let pos = pos_from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        let score = tb.probe_wdl(&pos);
        assert!(score.is_some());
        assert!(score.unwrap() > 0, "KQvK should be winning for white");
    }

    #[test]
    fn krvk_white_winning() {
        let tb = get_tb();
        if !tb.is_available() {
            return;
        }
        let pos = pos_from_fen("k7/8/8/8/8/8/8/K6R w - - 0 1");
        let score = tb.probe_wdl(&pos);
        assert!(score.is_some());
        assert!(score.unwrap() > 0, "KRvK should be winning for white");
    }

    #[test]
    fn starting_position_returns_none() {
        let tb = get_tb();
        if !tb.is_available() {
            return;
        }
        let pos = Chess::default();
        assert!(tb.probe_wdl(&pos).is_none());
    }
}
