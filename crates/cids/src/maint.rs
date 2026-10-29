use crate::types::{Chime, ChimeKind, Target, TestKind};

pub const LAMP_S: f64 = 3.;
pub const PA_S: f64 = 4.;
pub const DIRECTORS_S: f64 = 2.;

#[derive(Clone, Debug, PartialEq)]
pub enum TestResult {
    None,
    Running(TestKind),
    Pass(TestKind),
    Fail(TestKind, String),
    NotAllowed(TestKind),
}

#[derive(Clone, Debug)]
pub struct MaintOut {
    pub lamp_test: bool,
    pub pa_test: bool,
    pub chime: Option<Chime>,
    pub result: TestResult,
}

pub struct Maintenance {
    running: Option<(TestKind, f64)>,
    result: TestResult,
}

impl Maintenance {
    pub fn new() -> Self {
        Self { running: None, result: TestResult::None }
    }

    pub fn step(&mut self, request: Option<TestKind>, allowed: bool, directors: [bool; 3], channels: [f64; 4], dt: f64) -> MaintOut {
        let mut chime = None;
        if let Some(k) = request {
            if allowed && self.running.is_none() {
                let len = match k {
                    TestKind::Lamp => LAMP_S,
                    TestKind::Pa => PA_S,
                    TestKind::Directors => DIRECTORS_S,
                };
                self.running = Some((k, len));
                self.result = TestResult::Running(k);
                if k == TestKind::Pa {
                    chime = Some(Chime { target: Target::Cabin, kind: ChimeKind::TestTone });
                }
            } else if !allowed {
                self.result = TestResult::NotAllowed(k);
            }
        }
        if !allowed && self.running.is_some() {
            let k = self.running.take().map(|r| r.0).unwrap_or(TestKind::Lamp);
            self.result = TestResult::NotAllowed(k);
        }
        let mut lamp = false;
        let mut pa = false;
        if let Some((k, left)) = self.running {
            lamp = k == TestKind::Lamp;
            pa = k == TestKind::Pa;
            let left = left - dt;
            if left > 0. {
                self.running = Some((k, left));
            } else {
                self.running = None;
                self.result = match k {
                    TestKind::Lamp => TestResult::Pass(k),
                    TestKind::Pa => match channels[..3].iter().position(|c| *c >= 1.) {
                        Some(c) => TestResult::Fail(k, format!("PA {} CHANNEL", ["UPPER", "MAIN", "LOWER"][c])),
                        None => TestResult::Pass(k),
                    },
                    TestKind::Directors => match directors.iter().position(|d| !*d) {
                        Some(d) => TestResult::Fail(k, format!("DIRECTOR {}", d + 1)),
                        None => TestResult::Pass(k),
                    },
                };
            }
        }
        MaintOut { lamp_test: lamp, pa_test: pa, chime, result: self.result.clone() }
    }
}

impl Default for Maintenance {
    fn default() -> Self {
        Self::new()
    }
}
