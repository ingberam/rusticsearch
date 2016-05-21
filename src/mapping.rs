use std::collections::HashMap;

use rustc_serialize::json::Json;

use analysis::Analyzer;
use term::Term;


#[derive(Debug, PartialEq)]
pub enum FieldType {
    String,
    Binary,
    Number {
        size: u8,
        is_float: bool,
    },
    Boolean,
    Date,
}


impl Default for FieldType {
    fn default() -> FieldType {
        FieldType::String
    }
}


#[derive(Debug)]
pub struct FieldMapping {
    data_type: FieldType,
    is_stored: bool,
    pub is_in_all: bool,
    boost: f64,
    analyzer: Analyzer,
}


impl Default for FieldMapping {
    fn default() -> FieldMapping {
        FieldMapping {
            data_type: FieldType::default(),
            is_stored: false,
            is_in_all: true,
            boost: 1.0f64,
            analyzer: Analyzer::Standard,
        }
    }
}


impl FieldMapping {
    pub fn process_value_for_index(&self, value: Json) -> Option<Vec<Term>> {
        if value == Json::Null {
            return Some(vec![Term::Null]);
        }

        match self.data_type {
            FieldType::String => {
                match value {
                    Json::String(string) => {
                        // Analyzed strings become TSVectors. Unanalyzed strings become... strings
                        if self.analyzer == Analyzer::None {
                            Some(vec![Term::String(string)])
                        } else {
                            let tokens = self.analyzer.run(string).iter().cloned().map(|t| Term::String(t)).collect();
                            Some(tokens)
                        }
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
            FieldType::Number{size, is_float} => {
                match value {
                    // TODO check the numbers fit in "size"
                    Json::U64(num) => Some(vec![Term::U64(num)]),
                    Json::I64(num) => Some(vec![Term::I64(num)]),
                    Json::F64(num) => {
                        if !is_float {
                            return None;
                        }

                        Some(vec![Term::F64(num)])
                    }
                    _ => None,
                }
            }
            FieldType::Boolean => Some(vec![Term::Boolean(parse_boolean(&value))]),
            _ => None,
        }
    }

    pub fn process_value_for_query(&self, value: Json) -> Option<Vec<Term>> {
        // Currently not different from process_value_for_index
        self.process_value_for_index(value)
    }
}


#[derive(Debug)]
pub struct Mapping {
    pub fields: HashMap<String, FieldMapping>,
}

impl Mapping {
    pub fn from_json(json: &Json) -> Mapping {
        let json = json.as_object().unwrap();
        let properties_json = json.get("properties").unwrap().as_object().unwrap();

        // Parse fields
        let mut fields = HashMap::new();
        for (field_name, field_mapping_json) in properties_json.iter() {
            fields.insert(field_name.clone(),
                          FieldMapping::from_json(field_mapping_json));
        }

        Mapping { fields: fields }
    }
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


impl FieldMapping {
    pub fn from_json(json: &Json) -> FieldMapping {
        let json = json.as_object().unwrap();
        let mut field_mapping = FieldMapping::default();

        for (key, value) in json.iter() {
            match key.as_ref() {
                "type" => {
                    let type_name = value.as_string().unwrap();

                    field_mapping.data_type = match type_name.as_ref() {
                        "string" => FieldType::String,
                        "integer" => {
                            FieldType::Number {
                                size: 64,
                                is_float: false,
                            }
                        }
                        "boolean" => FieldType::Boolean,
                        "date" => FieldType::Date,
                        _ => {
                            // TODO; make this an error
                            warn!("unimplemented type name! {}", type_name);
                            FieldType::default()
                        }
                    };
                }
                "index" => {
                    let index = value.as_string().unwrap();
                    if index == "not_analyzed" {
                        field_mapping.analyzer = Analyzer::None;
                    } else {
                        // TODO: Implement other variants and make this an error
                        warn!("unimplemented index setting! {}", index);
                    }
                }
                "index_analyzer" => {
                    if let Some(ref s) = value.as_string() {
                        if s == &"edgengram_analyzer" {
                            field_mapping.analyzer = Analyzer::EdgeNGram;
                        }
                    }
                }
                "boost" => {
                    field_mapping.boost = value.as_f64().unwrap();
                }
                "store" => {
                    field_mapping.is_stored = parse_boolean(value);
                }
                "include_in_all" => {
                    field_mapping.is_in_all = parse_boolean(value);
                }
                _ => warn!("unimplemented field mapping key! {}", key),
            }

        }

        field_mapping
    }
}
