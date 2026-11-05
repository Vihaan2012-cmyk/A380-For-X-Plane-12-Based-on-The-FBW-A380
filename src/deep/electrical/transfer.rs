//! The AC transfer circuit: which source each main AC bus takes, and which line, bus tie and
//! isolation contactors that closes (A380 TTM ATA 24, "Table of Priority").
//!
//! Every engine generator feeds its own bus through its GLC and every external power its own bus
//! through its EPC. Anything else goes through the transfer circuit: BTC 1 and 2 join AC 1 and
//! AC 2 to the left bar, BTC 3 and 4 join AC 3 and AC 4 to the right bar, APU GEN A sits on the
//! left bar and APU GEN B on the right, the SIC joins the two bars, and BTC 5 and 6 join AC 2 to
//! AC 3 directly. The A380's generators never run in parallel, so a bar carries one source.

/// One AC source: engine generator or external power of the bus with that index (0 = AC 1),
/// or APU generator 0 (GEN A) or 1 (GEN B).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Gen(usize),
    Ext(usize),
    Apu(usize),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Available {
    pub gen: [bool; 4],
    pub ext: [bool; 4],
    pub apu: [bool; 2],
}

/// The sources and contactors this frame. `btc` is BTC 1..6.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plan {
    pub fed_by: [Option<Source>; 4],
    pub glc: [bool; 4],
    pub epc: [bool; 4],
    pub agc: [bool; 2],
    pub btc: [bool; 6],
    pub sic: bool,
}

use Source::{Apu, Ext, Gen};

/// The table of priority, one column per AC bus, highest priority first.
const NORMAL: [[Source; 8]; 4] = [
    [Gen(0), Ext(0), Apu(0), Apu(1), Gen(1), Ext(1), Gen(3), Ext(3)],
    [Gen(1), Ext(1), Apu(0), Apu(1), Gen(0), Ext(0), Gen(2), Ext(2)],
    [Gen(2), Ext(2), Apu(1), Apu(0), Gen(3), Ext(3), Gen(1), Ext(1)],
    [Gen(3), Ext(3), Apu(1), Apu(0), Gen(2), Ext(2), Gen(0), Ext(0)],
];

/// The particular table, for exactly one engine generator and one APU generator, except an
/// outer VFG with the APU generator on its own side (that case stays on [`NORMAL`], where the
/// APU generator takes the other three buses).
const PARTICULAR: [&[Source]; 4] = [
    &[Gen(0), Ext(0), Apu(0), Gen(1), Ext(1), Apu(1)],
    &[Gen(1), Ext(1), Gen(0), Ext(0), Apu(0), Gen(2), Ext(2), Apu(1)],
    &[Gen(2), Ext(2), Gen(3), Ext(3), Apu(1), Gen(1), Ext(1), Apu(0)],
    &[Gen(3), Ext(3), Apu(1), Gen(2), Ext(2), Apu(0)],
];

/// How many buses a source may already carry and still pick up another in a later row.
const SHARE_LIMIT: usize = 2;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const MID: usize = 2;

fn side_of_bus(bus: usize) -> usize {
    if bus < 2 {
        LEFT
    } else {
        RIGHT
    }
}

fn side_of(source: Source) -> usize {
    match source {
        Gen(i) | Ext(i) => side_of_bus(i),
        Apu(k) => k,
    }
}

fn is_available(a: &Available, source: Source) -> bool {
    match source {
        Gen(i) => a.gen[i],
        Ext(i) => a.ext[i],
        Apu(k) => a.apu[k],
    }
}

fn index(source: Source) -> usize {
    match source {
        Gen(i) => i,
        Ext(i) => 4 + i,
        Apu(k) => 8 + k,
    }
}

/// The bars (left, right, AC 2 - AC 3) a source on `bus` takes, and the contactors closing them.
struct Route {
    bars: [bool; 3],
    btc: [bool; 6],
    sic: bool,
}

fn route(source: Source, bus: usize) -> Option<Route> {
    let mut r = Route { bars: [false; 3], btc: [false; 6], sic: false };
    match source {
        Gen(j) | Ext(j) if j == bus => return None,
        Gen(j) | Ext(j) => {
            if (j == 1 && bus == 2) || (j == 2 && bus == 1) {
                r.bars[MID] = true;
                r.btc[4] = true;
                r.btc[5] = true;
            } else {
                r.btc[j] = true;
                r.btc[bus] = true;
                r.bars[side_of_bus(j)] = true;
                r.bars[side_of_bus(bus)] = true;
                r.sic = side_of_bus(j) != side_of_bus(bus);
            }
        }
        Apu(k) => {
            r.btc[bus] = true;
            r.bars[k] = true;
            r.bars[side_of_bus(bus)] = true;
            r.sic = k != side_of_bus(bus);
        }
    }
    Some(r)
}

/// Picks every bus's source and the contactors that connect it. With BUS TIE OFF the transfer
/// circuit stays open and every bus takes only its own generator or external power.
pub fn plan(a: &Available, bus_tie_auto: bool) -> Plan {
    let mut p = Plan::default();
    let count = a.gen.iter().chain(&a.ext).chain(&a.apu).filter(|&&x| x).count();
    let only_side = (count == 1).then(|| {
        let s = (0..4).find(|&i| a.gen[i]).map(Gen).or_else(|| (0..4).find(|&i| a.ext[i]).map(Ext)).unwrap_or_else(|| Apu((0..2).find(|&k| a.apu[k]).unwrap_or(0)));
        side_of(s)
    });
    let gens = a.gen.iter().filter(|&&x| x).count();
    let apus = a.apu.iter().filter(|&&x| x).count();
    let outer_with_own_apu = (a.gen[0] && a.apu[0]) || (a.gen[3] && a.apu[1]);
    let particular = !a.ext.iter().any(|&x| x) && gens == 1 && apus == 1 && !outer_with_own_apu;
    let column = |bus: usize, row: usize| -> Option<Source> {
        if particular {
            PARTICULAR[bus].get(row).copied()
        } else {
            NORMAL[bus].get(row).copied()
        }
    };

    let mut carried = [0usize; 10];
    let mut bar_source: [Option<Source>; 3] = [None; 3];
    for row in 0..8 {
        let before = carried;
        for bus in 0..4 {
            if p.fed_by[bus].is_some() {
                continue;
            }
            let Some(source) = column(bus, row) else { continue };
            if !is_available(a, source) || before[index(source)] >= SHARE_LIMIT {
                continue;
            }
            if only_side.is_some_and(|side| side != side_of_bus(bus)) {
                continue;
            }
            if let Gen(j) | Ext(j) = source {
                if j != bus && p.fed_by[j] != Some(source) {
                    continue;
                }
            }
            match route(source, bus) {
                None => {}
                Some(r) => {
                    if !bus_tie_auto {
                        continue;
                    }
                    let clash = (0..3).any(|b| r.bars[b] && bar_source[b].is_some_and(|s| s != source));
                    if clash {
                        continue;
                    }
                    for b in 0..3 {
                        if r.bars[b] {
                            bar_source[b] = Some(source);
                        }
                    }
                    for (c, &on) in p.btc.iter_mut().zip(&r.btc) {
                        *c |= on;
                    }
                    p.sic |= r.sic;
                }
            }
            p.fed_by[bus] = Some(source);
            carried[index(source)] += 1;
        }
    }
    for bus in 0..4 {
        match p.fed_by[bus] {
            Some(Gen(j)) if j == bus => p.glc[bus] = true,
            Some(Ext(j)) if j == bus => p.epc[bus] = true,
            _ => {}
        }
    }
    for bus in 0..4 {
        match p.fed_by[bus] {
            Some(Gen(j)) if j != bus => p.glc[j] = true,
            Some(Ext(j)) if j != bus => p.epc[j] = true,
            Some(Apu(k)) => p.agc[k] = true,
            _ => {}
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avail(gen: [bool; 4], ext: [bool; 4], apu: [bool; 2]) -> Available {
        Available { gen, ext, apu }
    }

    #[test]
    fn four_engine_generators_each_carry_their_own_bus() {
        let p = plan(&avail([true; 4], [false; 4], [false; 2]), true);
        assert_eq!(p.fed_by, [Some(Gen(0)), Some(Gen(1)), Some(Gen(2)), Some(Gen(3))]);
        assert_eq!(p.btc, [false; 6]);
        assert!(!p.sic);
    }

    #[test]
    fn one_external_power_carries_its_own_side_only() {
        let p = plan(&avail([false; 4], [false, true, false, false], [false; 2]), true);
        assert_eq!(p.fed_by, [Some(Ext(1)), Some(Ext(1)), None, None]);
        assert!(p.btc[0] && p.btc[1] && !p.sic);
    }

    #[test]
    fn two_external_powers_carry_the_whole_network() {
        let p = plan(&avail([false; 4], [false, true, true, false], [false; 2]), true);
        assert_eq!(p.fed_by, [Some(Ext(1)), Some(Ext(1)), Some(Ext(2)), Some(Ext(2))]);
        assert!(!p.sic, "each external power feeds its own side");
    }

    #[test]
    fn with_ext_2_and_3_each_apu_generator_takes_the_outer_bus_on_its_side() {
        let p = plan(&avail([false; 4], [false, true, true, false], [true; 2]), true);
        assert_eq!(p.fed_by, [Some(Apu(0)), Some(Ext(1)), Some(Ext(2)), Some(Apu(1))]);
        assert_eq!(p.agc, [true, true]);
        assert!(p.btc[0] && p.btc[3] && !p.btc[1] && !p.btc[2] && !p.sic);
    }

    #[test]
    fn losing_vfg_1_and_2_feeds_ac_1_from_ac_4_through_the_sic_and_ac_2_from_ac_3() {
        let p = plan(&avail([false, false, true, true], [false; 4], [false; 2]), true);
        assert_eq!(p.fed_by, [Some(Gen(3)), Some(Gen(2)), Some(Gen(2)), Some(Gen(3))]);
        assert!(p.btc[3] && p.sic && p.btc[0], "AC 4 through BTC 4, the SIC and BTC 1");
        assert!(p.btc[4] && p.btc[5], "AC 3 through BTC 5 and 6");
        assert!(!p.btc[1] && !p.btc[2]);
    }

    #[test]
    fn a_single_generator_carries_half_the_network() {
        let p = plan(&avail([true, false, false, false], [false; 4], [false; 2]), true);
        assert_eq!(p.fed_by, [Some(Gen(0)), Some(Gen(0)), None, None]);
    }

    #[test]
    fn vfg_1_with_apu_gen_a_leaves_the_apu_three_buses() {
        let p = plan(&avail([true, false, false, false], [false; 4], [true, false]), true);
        assert_eq!(p.fed_by, [Some(Gen(0)), Some(Apu(0)), Some(Apu(0)), Some(Apu(0))]);
        assert!(p.sic);
    }

    #[test]
    fn vfg_1_with_apu_gen_b_splits_the_network_by_side() {
        let p = plan(&avail([true, false, false, false], [false; 4], [false, true]), true);
        assert_eq!(p.fed_by, [Some(Gen(0)), Some(Gen(0)), Some(Apu(1)), Some(Apu(1))]);
        assert!(!p.sic);
    }

    #[test]
    fn vfg_2_with_apu_gen_a_feeds_ac_3_across_the_middle_bar() {
        let p = plan(&avail([false, true, false, false], [false; 4], [true, false]), true);
        assert_eq!(p.fed_by, [Some(Apu(0)), Some(Gen(1)), Some(Gen(1)), Some(Apu(0))]);
        assert!(p.btc[4] && p.btc[5] && p.sic && p.btc[0] && p.btc[3]);
    }

    #[test]
    fn both_apu_generators_alone_carry_the_whole_network_on_the_ground() {
        let p = plan(&avail([false; 4], [false; 4], [true; 2]), true);
        assert_eq!(p.fed_by, [Some(Apu(0)), Some(Apu(0)), Some(Apu(1)), Some(Apu(1))]);
        assert!(!p.sic);
    }

    #[test]
    fn bus_tie_off_keeps_every_bus_on_its_own_source() {
        let p = plan(&avail([true, false, false, true], [false; 4], [true; 2]), false);
        assert_eq!(p.fed_by, [Some(Gen(0)), None, None, Some(Gen(3))]);
        assert_eq!(p.btc, [false; 6]);
        assert!(!p.sic && p.agc == [false, false]);
    }

    #[test]
    fn no_bar_ever_carries_two_sources() {
        for bits in 0u32..1024 {
            let b = |i: u32| bits & (1 << i) != 0;
            let a = avail([b(0), b(1), b(2), b(3)], [b(4), b(5), b(6), b(7)], [b(8), b(9)]);
            let p = plan(&a, true);
            for bus in 0..4 {
                if let Some(s) = p.fed_by[bus] {
                    assert!(is_available(&a, s), "{bits:010b}: bus {bus} on an unavailable {s:?}");
                }
            }
            let left: Vec<Source> = (0..2).filter(|&i| p.btc[i]).filter_map(|i| p.fed_by[i]).chain((0..2).filter(|&k| p.agc[k] && k == 0).map(Apu)).collect();
            let right: Vec<Source> = (2..4).filter(|&i| p.btc[i]).filter_map(|i| p.fed_by[i]).chain((0..2).filter(|&k| p.agc[k] && k == 1).map(Apu)).collect();
            let mut on_bar = left.clone();
            if p.sic {
                on_bar.extend(&right);
                assert!(on_bar.windows(2).all(|w| w[0] == w[1]), "{bits:010b}: the SIC parallels {on_bar:?}");
            } else {
                assert!(left.windows(2).all(|w| w[0] == w[1]) && right.windows(2).all(|w| w[0] == w[1]), "{bits:010b}: {left:?} / {right:?}");
            }
        }
    }
}
