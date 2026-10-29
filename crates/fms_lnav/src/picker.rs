use crate::input::{HoldKind, LegSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegKind {
    If,
    Af,
    Ca,
    Cd,
    Cf,
    Ci,
    Cr,
    Df,
    Fa,
    Fc,
    Fd,
    Fm,
    Ha,
    Hf,
    Hm,
    Pi,
    Rf,
    Tf,
    Vm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind {
    FixedRadius,
    PathCapture,
    CourseCapture,
    DirectToFix,
    DmeArc,
    HoldEntry,
}

impl LegKind {
    fn is_hold(self) -> bool {
        matches!(self, LegKind::Ha | LegKind::Hf | LegKind::Hm)
    }
}

impl LegSpec {
    pub fn kind(&self) -> LegKind {
        match self {
            LegSpec::If => LegKind::If,
            LegSpec::Tf { .. } => LegKind::Tf,
            LegSpec::Df => LegKind::Df,
            LegSpec::Cf { .. } => LegKind::Cf,
            LegSpec::Rf { .. } => LegKind::Rf,
            LegSpec::Ci { .. } => LegKind::Ci,
            LegSpec::Fa { .. } => LegKind::Fa,
            LegSpec::Fc { .. } => LegKind::Fc,
            LegSpec::Fm { .. } => LegKind::Fm,
            LegSpec::Ca { .. } => LegKind::Ca,
            LegSpec::Cr { .. } => LegKind::Cr,
            LegSpec::Vm { .. } => LegKind::Vm,
            LegSpec::Cd { .. } => LegKind::Cd,
            LegSpec::Fd { .. } => LegKind::Fd,
            LegSpec::Af { .. } => LegKind::Af,
            LegSpec::Pi { .. } => LegKind::Pi,
            LegSpec::Hold { kind: HoldKind::Ha, .. } => LegKind::Ha,
            LegSpec::Hold { kind: HoldKind::Hf, .. } => LegKind::Hf,
            LegSpec::Hold { kind: HoldKind::Hm, .. } => LegKind::Hm,
        }
    }
}

use LegKind as L;
use TransitionKind as T;

pub fn pick(from: LegKind, to: LegKind, same_waypoint: bool) -> Option<TransitionKind> {
    let course_capture_targets = |to: LegKind| matches!(to, L::Ca | L::Cd | L::Ci | L::Cr | L::Vm);
    match from {
        L::If => None,
        L::Af => match to {
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fc | L::Fd | L::Fa | L::Fm => Some(T::PathCapture),
            t if t.is_hold() => Some(T::HoldEntry),
            L::Tf => Some(T::DmeArc),
            _ => None,
        },
        L::Ca => match to {
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fc | L::Fd | L::Fm | L::Tf => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Cd => match to {
            L::Af => Some(T::DmeArc),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fc | L::Fd | L::Fm => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Cf => match to {
            L::Af => Some(T::DmeArc),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fm | L::Pi | L::Tf => Some(T::FixedRadius),
            L::Df => Some(T::DirectToFix),
            L::Fc | L::Fd => Some(T::PathCapture),
            t if t.is_hold() => Some(T::HoldEntry),
            _ => None,
        },
        L::Ci => match to {
            L::Af => Some(T::DmeArc),
            L::Cf | L::Fa | L::Fm | L::Tf => Some(T::FixedRadius),
            L::Fc | L::Fd => Some(T::PathCapture),
            _ => None,
        },
        L::Df => match to {
            L::Af => Some(T::DmeArc),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fm | L::Pi | L::Tf => Some(T::FixedRadius),
            L::Df => Some(T::DirectToFix),
            L::Fc | L::Fd => Some(if same_waypoint { T::FixedRadius } else { T::PathCapture }),
            t if t.is_hold() => Some(T::HoldEntry),
            _ => None,
        },
        h if h.is_hold() => match to {
            L::Af | L::Cf | L::Fa | L::Fc | L::Fd | L::Fm | L::Tf => Some(T::PathCapture),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Pi => None,
        L::Rf => match to {
            L::Ca | L::Cd | L::Ci => Some(T::CourseCapture),
            L::Fa | L::Fc | L::Fd | L::Fm => Some(T::PathCapture),
            t if t.is_hold() => Some(T::HoldEntry),
            _ => None,
        },
        L::Tf => match to {
            L::Af => Some(T::DmeArc),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fm | L::Pi | L::Tf => Some(T::FixedRadius),
            L::Df => Some(T::DirectToFix),
            L::Fc | L::Fd => Some(if same_waypoint { T::FixedRadius } else { T::PathCapture }),
            t if t.is_hold() => Some(T::HoldEntry),
            _ => None,
        },
        L::Cr => match to {
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fc | L::Fd | L::Fm => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Fa => match to {
            t if course_capture_targets(t) => Some(T::CourseCapture),
            L::Cf | L::Fa | L::Fm => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Fc => match to {
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fm => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Fd => match to {
            L::Af => Some(T::DmeArc),
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Cf | L::Fm => Some(T::PathCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Fm => match to {
            L::Ca | L::Cd | L::Ci | L::Cr | L::Vm => Some(T::CourseCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        L::Vm => match to {
            L::Ca | L::Cd | L::Ci | L::Cr => Some(T::CourseCapture),
            L::Df => Some(T::DirectToFix),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xf_pairs_follow_the_table() {
        assert_eq!(pick(L::Tf, L::Tf, false), Some(T::FixedRadius));
        assert_eq!(pick(L::Cf, L::Ci, false), Some(T::CourseCapture));
        assert_eq!(pick(L::Ci, L::Cf, false), Some(T::FixedRadius));
        assert_eq!(pick(L::Tf, L::Df, false), Some(T::DirectToFix));
        assert_eq!(pick(L::Df, L::Fc, true), Some(T::FixedRadius));
        assert_eq!(pick(L::Df, L::Fc, false), Some(T::PathCapture));
        assert_eq!(pick(L::Cf, L::Hm, false), Some(T::HoldEntry));
        assert_eq!(pick(L::Rf, L::Tf, false), None);
        assert_eq!(pick(L::Vm, L::Fa, false), None);
        assert_eq!(pick(L::Ha, L::Tf, false), Some(T::PathCapture));
    }
}
