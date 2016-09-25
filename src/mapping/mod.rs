pub mod build;
pub mod parse;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use rustc_serialize::json::Json;
use chrono::{DateTime, UTC};
use abra::{Term, Token};
use abra::analysis::AnalyzerSpec;
use abra::analysis::tokenizers::TokenizerSpec;
use abra::analysis::filters::FilterSpec;
use abra::similarity::SimilarityModel;
use abra::schema::FieldRef;

use analysis::registry::AnalyzerRegistry;


// TEMPORARY
fn get_standard_analyzer() -> AnalyzerSpec {
    AnalyzerSpec {
        tokenizer: TokenizerSpec::Standard,
        filters: vec![
            FilterSpec::Lowercase,
            FilterSpec::ASCIIFolding,
        ]
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Boolean,
    Date,
}


impl Default for FieldType {
    fn default() -> FieldType {
        FieldType::String
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct FieldSearchOptions {
    pub analyzer: AnalyzerSpec,
    pub similarity_model: SimilarityModel,
}


impl Default for FieldSearchOptions {
    fn default() -> FieldSearchOptions {
        FieldSearchOptions {
            analyzer: get_standard_analyzer(),
            similarity_model: SimilarityModel::Bm25 {
                k1: 1.2,
                b: 0.75,
            },
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct FieldMapping {
    pub data_type: FieldType,
    pub index_ref: Option<FieldRef>,
    is_stored: bool,
    pub is_in_all: bool,
    boost: f64,
    base_analyzer: AnalyzerSpec,
    index_analyzer: Option<AnalyzerSpec>,
    search_analyzer: Option<AnalyzerSpec>,
}


impl Default for FieldMapping {
    fn default() -> FieldMapping {
        FieldMapping {
            data_type: FieldType::default(),
            index_ref: None,
            is_stored: false,
            is_in_all: true,
            boost: 1.0f64,
            base_analyzer: get_standard_analyzer(),
            index_analyzer: None,
            search_analyzer: None,
        }
    }
}


impl FieldMapping {
    pub fn index_analyzer(&self) -> &AnalyzerSpec {
        if let Some(ref index_analyzer) = self.index_analyzer {
            index_analyzer
        } else {
            &self.base_analyzer
        }
    }

    pub fn search_analyzer(&self) -> &AnalyzerSpec {
        if let Some(ref search_analyzer) = self.search_analyzer {
            search_analyzer
        } else {
            &self.base_analyzer
        }
    }

    pub fn get_search_options(&self) -> FieldSearchOptions {
        FieldSearchOptions {
            analyzer: self.search_analyzer().clone(),
            .. FieldSearchOptions::default()
        }
    }

    pub fn process_value_for_index(&self, value: Json) -> Option<Vec<Token>> {
        if value == Json::Null {
            return None;
        }

        match self.data_type {
            FieldType::String => {
                match value {
                    Json::String(string) => {
                        // Analyze string
                        let tokens = self.index_analyzer().initialise(&string);
                        Some(tokens.collect::<Vec<Token>>())
                    }
                    Json::I64(num) => self.process_value_for_index(Json::String(num.to_string())),
                    Json::U64(num) => self.process_value_for_index(Json::String(num.to_string())),
                    Json::F64(num) => self.process_value_for_index(Json::String(num.to_string())),
                    Json::Array(array) => {
                        // Pack any strings into a vec, ignore nulls. Quit if we see anything else
                        let mut strings = Vec::new();

                        for item in array {
                            match item {
                                Json::String(string) => strings.push(string),
                                Json::Null => {}
                                _ => {
                                    return None;
                                }
                            }
                        }

                        self.process_value_for_index(Json::String(strings.join(" ")))
                    }
                    _ => None,
                }
            }
            FieldType::Integer => {
                match value {
                    Json::U64(num) => Some(vec![Token{term: Term::I64(num as i64), position: 1}]),
                    Json::I64(num) => Some(vec![Token{term: Term::I64(num), position: 1}]),
                    _ => None,
                }
            }
            FieldType::Boolean => Some(vec![Token{term: Term::Boolean(parse_boolean(&value)), position: 1}]),
            FieldType::Date => {
                match value {
                    Json::String(string) => {
                        let date_parsed = match string.parse::<DateTime<UTC>>() {
                            Ok(date_parsed) => date_parsed,
                            Err(_) => {
                                // TODO: Handle this properly
                                return None;
                            }
                        };

                        Some(vec![Token{term: Term::DateTime(date_parsed), position: 1}])
                    }
                    Json::U64(_) => {
                        // TODO needs to be interpreted as milliseconds since epoch
                        // This would really help: https://github.com/lifthrasiir/rust-chrono/issues/74
                        None
                    }
                    _ => None
                }
            }
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct Mapping {
    pub fields: HashMap<String, FieldMapping>,
}


#[derive(Debug)]
pub struct MappingRegistry {
    mappings: HashMap<String, Mapping>,
}


fn parse_boolean(json: &Json) -> bool {
    match *json {
        Json::Boolean(val) => val,
        Json::String(ref s) => {
            match s.as_ref() {
                "yes" => true,
                "no" => false,
                _ => {
                    warn!("bad boolean value {:?}", s);
                    false
                }
           }
        }
        _ => {
            // TODO: Raise error
            warn!("bad boolean value {:?}", json);
            false
        }
    }
}



impl MappingRegistry {
    pub fn new() -> MappingRegistry {
        MappingRegistry {
            mappings: HashMap::new(),
        }
    }

    pub fn get_field(&self, name: &str) -> Option<&FieldMapping> {
        for mapping in self.mappings.values() {
            if let Some(ref field_mapping) = mapping.fields.get(name) {
                return Some(field_mapping);
            }
        }

        None
    }
}


impl Deref for MappingRegistry {
    type Target = HashMap<String, Mapping>;

    fn deref(&self) -> &HashMap<String, Mapping> {
        &self.mappings
    }
}


impl DerefMut for MappingRegistry {
    fn deref_mut(&mut self) -> &mut HashMap<String, Mapping> {
        &mut self.mappings
    }
}
