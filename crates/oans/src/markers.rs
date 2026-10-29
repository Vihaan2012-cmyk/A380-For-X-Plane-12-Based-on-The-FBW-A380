use crate::geo::Position;
use crate::labels::{feature_type as ft, Label, LabelStyle};
use crate::oanc::{Feature, Geometry};
use crate::turf::{along_degrees, length_degrees};

pub const TAXIWAY_GUIDANCE_LINE: u8 = 16;
const MAX_SYMBOL_DIST_NEIGHBORHOOD_SEARCH: f64 = 20.0;
const TAXIWAY_SYMBOL_SPACING: f64 = 250.0;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkerFeature {
    pub id: Option<i64>,
    pub feattype: u8,
    pub idlin: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub id: u32,
    pub position: Position,
    pub feature: Option<MarkerFeature>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Cross,
    Flag,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolsForFeatureIds {
    pub feature_ids_with_crosses: Vec<i64>,
    pub feature_ids_with_flags: Vec<i64>,
}

#[derive(Default)]
pub struct MarkerManager {
    crosses: Vec<Marker>,
    flags: Vec<Marker>,
    next_cross_id: u32,
    next_flag_id: u32,
    pub published: Vec<SymbolsForFeatureIds>,
}

fn marker_feature(f: &Feature) -> MarkerFeature {
    MarkerFeature { id: f.props.id, feattype: f.feattype, idlin: f.props.idlin.clone() }
}

fn unique_ids(list: &[Marker]) -> Vec<i64> {
    let mut out = Vec::new();
    for id in list.iter().filter_map(|m| m.feature.as_ref().and_then(|f| f.id)) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

impl MarkerManager {
    pub fn new() -> Self {
        let mut m = Self::default();
        m.update_symbols_for_feature_ids();
        m.update_symbols_for_feature_ids();
        m
    }

    pub fn crosses(&self) -> &[Marker] {
        &self.crosses
    }

    pub fn flags(&self) -> &[Marker] {
        &self.flags
    }

    pub fn labels(&self) -> Vec<Label> {
        let mk = |m: &Marker, style| Label {
            text: m.id.to_string(),
            style,
            position: m.position,
            rotation: Some(0.0),
            feattype: m.feature.as_ref().map(|f| f.feattype),
            axis_ends: None,
            termref: None,
        };
        self.crosses.iter().map(|m| mk(m, LabelStyle::CrossSymbol)).chain(self.flags.iter().map(|m| mk(m, LabelStyle::FlagSymbol))).collect()
    }

    pub fn symbols_for_feature_ids(&self) -> SymbolsForFeatureIds {
        SymbolsForFeatureIds { feature_ids_with_crosses: unique_ids(&self.crosses), feature_ids_with_flags: unique_ids(&self.flags) }
    }

    fn update_symbols_for_feature_ids(&mut self) {
        let s = self.symbols_for_feature_ids();
        self.published.push(s);
    }

    pub fn add(&mut self, kind: Symbol, position: Position, feature: Option<MarkerFeature>) {
        let (list, next) = match kind {
            Symbol::Cross => (&mut self.crosses, &mut self.next_cross_id),
            Symbol::Flag => (&mut self.flags, &mut self.next_flag_id),
        };
        list.push(Marker { id: *next, position, feature });
        *next += 1;
        self.update_symbols_for_feature_ids();
    }

    pub fn remove(&mut self, kind: Symbol, id: u32) {
        let list = match kind {
            Symbol::Cross => &mut self.crosses,
            Symbol::Flag => &mut self.flags,
        };
        if let Some(i) = list.iter().position(|m| m.id == id) {
            list.remove(i);
            self.update_symbols_for_feature_ids();
        }
    }

    pub fn erase_all(&mut self, kind: Symbol) {
        match kind {
            Symbol::Cross => self.crosses.clear(),
            Symbol::Flag => self.flags.clear(),
        }
        self.update_symbols_for_feature_ids();
    }

    pub fn add_symbol_at_feature(&mut self, kind: Symbol, features: &[Feature], id: i64, feattype: u8) {
        let matching: Vec<&Feature> = features.iter().filter(|f| f.props.id == Some(id) && f.feattype == feattype).collect();
        if feattype == TAXIWAY_GUIDANCE_LINE {
            let Some(first) = matching.first() else { return };
            let idlin = first.props.idlin.clone();
            let lines: Vec<&Feature> = features
                .iter()
                .filter(|f| f.props.idlin == idlin && (f.feattype == TAXIWAY_GUIDANCE_LINE || f.feattype == ft::RUNWAY_EXIT_LINE))
                .collect();
            for tw in lines {
                let Geometry::LineString(line) = &tw.geometry else { continue };
                let tw_length = length_degrees(line);
                if tw_length > TAXIWAY_SYMBOL_SPACING {
                    let mut along = 0.0;
                    while along < tw_length {
                        self.add(kind, along_degrees(line, along.min(tw_length)), Some(marker_feature(tw)));
                        along += TAXIWAY_SYMBOL_SPACING;
                    }
                } else {
                    self.add(kind, line[0], Some(marker_feature(tw)));
                    self.add(kind, line[line.len() - 1], Some(marker_feature(tw)));
                }
            }
        } else if feattype == ft::RUNWAY_THRESHOLD || feattype == ft::PARKING_STAND_LOCATION {
            let geo: Vec<&Feature> = features
                .iter()
                .filter(|f| f.props.id == Some(id) && (f.feattype == ft::RUNWAY_THRESHOLD || f.feattype == ft::PARKING_STAND_LOCATION))
                .collect();
            if let Some(f) = geo.first() {
                if let Geometry::Point(p) = f.geometry {
                    self.add(kind, p, Some(marker_feature(f)));
                }
            }
        }
    }

    pub fn remove_symbol_at_feature(&mut self, kind: Symbol, id: i64, feattype: u8) {
        let symbols = match kind {
            Symbol::Cross => self.crosses.clone(),
            Symbol::Flag => self.flags.clone(),
        };
        let is_taxiway = symbols.iter().any(|m| m.feature.as_ref().is_some_and(|f| f.id == Some(id) && f.feattype == TAXIWAY_GUIDANCE_LINE));
        let ids: Vec<u32> = if is_taxiway {
            let mut name = String::new();
            for m in &symbols {
                if let Some(f) = m.feature.as_ref().filter(|f| f.id == Some(id) && f.feattype == feattype) {
                    name = f.idlin.clone().unwrap_or_default();
                    break;
                }
            }
            symbols.iter().filter(|m| m.feature.as_ref().is_some_and(|f| f.idlin.as_deref() == Some(name.as_str()))).map(|m| m.id).collect()
        } else {
            symbols.iter().filter(|m| m.feature.as_ref().is_some_and(|f| f.id == Some(id) && f.feattype == feattype)).map(|m| m.id).collect()
        };
        for i in ids {
            self.remove(kind, i);
        }
    }

    pub fn find_symbol_at_cursor(&self, position: Position) -> (Option<u32>, Option<u32>) {
        let near = |m: &&Marker| (position[0] - m.position[0]).hypot(position[1] - m.position[1]) < MAX_SYMBOL_DIST_NEIGHBORHOOD_SEARCH;
        (self.crosses.iter().find(near).map(|m| m.id), self.flags.iter().find(near).map(|m| m.id))
    }

    pub fn find_symbol_at_feature(&self, id: i64, feattype: u8) -> (bool, bool) {
        let has = |list: &[Marker]| list.iter().any(|m| m.feature.as_ref().is_some_and(|f| f.id == Some(id) && f.feattype == feattype));
        (has(&self.crosses), has(&self.flags))
    }
}
