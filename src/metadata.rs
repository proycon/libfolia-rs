use std::collections::HashMap;
use std::fmt;

use crate::common::*;
use crate::error::*;
use crate::types::*;
use crate::element::*;
use crate::store::*;


pub struct Declaration {
    pub annotationtype: AnnotationType,
    pub set: Option<String>,
    pub alias: Option<String>,
    pub processors: Vec<ProcKey>,
}

impl Declaration {
    pub fn new(annotationtype: AnnotationType, set: Option<String>, alias: Option<String>) -> Declaration {
        Declaration { annotationtype: annotationtype, set: set, alias: alias, processors: vec![] }
    }
}

impl CheckEncoded for Declaration { }

impl CheckEncoded for String { }

impl MaybeIdentifiable for Declaration {
    fn id(&self) -> Option<String> {
        Some(DeclarationStore::index_id(self.annotationtype,&self.set))
    }
}


#[derive(Default)]
pub struct ClassStore {
    items: Vec<Option<Box<String>>>, //heap-allocated
    index: HashMap<String,ClassKey>
}

impl MaybeIdentifiable for String {
    fn id(&self) -> Option<String> {
        Some(self.to_string())
    }
}


impl Store<String,ClassKey> for ClassStore {
    fn items_mut(&mut self) -> &mut Vec<Option<Box<String>>> {
        &mut self.items
    }
    fn index_mut(&mut self) -> &mut HashMap<String,ClassKey> {
        &mut self.index
    }

    fn items(&self) -> &Vec<Option<Box<String>>> {
        &self.items
    }
    fn index(&self) -> &HashMap<String,ClassKey> {
        &self.index
    }
    fn iter(&self) -> std::slice::Iter<Option<Box<String>>> {
        self.items.iter()
    }
}


#[derive(Default)]
pub struct DeclarationStore {
    items: Vec<Option<Box<Declaration>>>, //heap-allocated
    index: HashMap<String,DecKey>,
    classes: Option<ClassStore>
}

impl DeclarationStore {

    ///Create a id to use with the index
    pub fn index_id(annotationtype: AnnotationType, set: &Option<String>) -> String {
        if let Some(set) = set {
            format!("{}/{}", annotationtype, set)
        } else {
            format!("{}", annotationtype)
        }
    }

    ///Returns a vector of boolean, indicating if the declaration is a default or not. Can be
    ///indexed with DecKey
    pub fn default_mask(&self) -> Vec<bool> {
        let mut typecount: HashMap<AnnotationType,usize> = HashMap::new();
        for declaration in self.items.iter() {
            if let Some(declaration) = declaration {
                if let Some(count) = typecount.get_mut(&declaration.annotationtype) {
                    *count += 1;
                } else {
                    typecount.insert(declaration.annotationtype, 1);
                }
            }
        }
        let mut mask: Vec<bool> = Vec::with_capacity(self.items.len());
        for declaration in self.items.iter() {
            if let Some(declaration) = declaration {
                mask.push( typecount.get(&declaration.annotationtype) == Some(&1) );
            } else {
                mask.push(false);
            }
        }
        mask
    }

    ///Declares a new annotation type and set or returns the key of an existing one
    pub fn declare(&mut self, annotationtype: AnnotationType, set: &Option<String>, alias: &Option<String>) -> Result<DecKey,FoliaError> {
        //first we simply check the index
        if let Some(found_key) = self.id_to_key(Self::index_id(annotationtype, set).as_str()) {
            return Ok(found_key);
        }

        //If not found, we search for a default
        if let Some(default_key) = self.get_default_key(annotationtype) {
            if let Some(declaration) = self.get(default_key) {
                if set.is_some() {
                    //there is an explicit set defined, only return the default if the sets are not
                    //in conflict
                    if let Some(declared_set) = &declaration.set {
                        if *declared_set == *set.as_ref().unwrap() {
                            return Ok(default_key);
                        }
                    }
                } else {
                    //no set defined, that means we inherit the default set
                    return Ok(default_key);
                }
            }
        }

        //if we reach this point we have no defaults and add a new declaration
        let added_key = self.add(Declaration::new(annotationtype, set.clone(), alias.clone()))?;
        Ok(added_key)
    }

    ///Retrieves the key for the default annotation for the given annotationtype (if there is a
    ///default)
    pub fn get_default_key(&self, annotationtype: AnnotationType) -> Option<DecKey> {
        let matches: Vec<usize> = self.items.iter().enumerate().filter_map(|(index, declaration)|  {
            if let Some(declaration) = declaration {
                if declaration.annotationtype  == annotationtype {
                    Some(index)
                } else {
                    None
                }
            } else {
                None
            }
        }).collect();
        if matches.len() == 1 {
            Some(matches[0] as DecKey)
        } else {
            None
        }
    }

}

impl Store<Declaration,DecKey> for DeclarationStore {
    fn items_mut(&mut self) -> &mut Vec<Option<Box<Declaration>>> {
        &mut self.items
    }
    fn index_mut(&mut self) -> &mut HashMap<String,DecKey> {
        &mut self.index
    }

    fn items(&self) -> &Vec<Option<Box<Declaration>>> {
        &self.items
    }
    fn index(&self) -> &HashMap<String,DecKey> {
        &self.index
    }
    fn iter(&self) -> std::slice::Iter<Option<Box<Declaration>>> {
        self.items.iter()
    }
}

#[derive(Default)]
pub struct ProvenanceStore {
    items: Vec<Option<Box<Processor>>>, //heap-allocated
    index: HashMap<String,ProcKey>,
    pub chain: Vec<ProcKey>,
}

impl Store<Processor,ProcKey> for ProvenanceStore {
    fn items_mut(&mut self) -> &mut Vec<Option<Box<Processor>>> {
        &mut self.items
    }
    fn index_mut(&mut self) -> &mut HashMap<String,ProcKey> {
        &mut self.index
    }

    fn items(&self) -> &Vec<Option<Box<Processor>>> {
        &self.items
    }
    fn index(&self) -> &HashMap<String,ProcKey> {
        &self.index
    }
    fn iter(&self) -> std::slice::Iter<Option<Box<Processor>>> {
        self.items.iter()
    }
}

impl ProvenanceStore {
    pub fn add_to_chain(&mut self, child: Processor) -> Result<ProcKey,FoliaError> {
        let child_key = self.add(child);
        if let Ok(child_key) = child_key {
            self.chain.push(child_key);
        }
        child_key
    }

    ///Adds a processor as a child of another, this is a higher-level function that/
    ///takes care of adding and attaching for you.
    pub fn add_to(&mut self, parent_key: ProcKey, child: Processor) -> Result<ProcKey,FoliaError> {
        let child_key = self.add(child);
        if let Ok(child_key) = child_key {
            self.attach(parent_key, child_key)?;
        }
        child_key
    }

    ///Adds the processor element to the parent element, automatically takes care
    ///of removing the old parent (if any).
    pub fn attach(&mut self, parent_key: ProcKey, child_key: ProcKey) -> Result<(),FoliaError> {
        //ensure the parent exists
        if !self.get(parent_key).is_some() {
            return Err(FoliaError::InternalError(format!("Parent does not exist: {}", parent_key)));
        };

        if let Some(child) = self.get_mut(child_key) {
            //add the new parent and return the old parent
            child.parent = Some(parent_key);
        } else {
            //child does not exist
            return Err(FoliaError::InternalError(format!("Child does not exist: {}", child_key)));
        };

        if let Some(parent) = self.get_mut(parent_key) {
            parent.processors.push(child_key);
        }

        Ok(())
    }
}


#[derive(Debug,PartialEq,Clone,Copy)]
pub enum ProcessorType {
    Auto,
    Manual,
    Generator,
    DataSource,
}

impl ProcessorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessorType::Auto => "auto",
            ProcessorType::Manual => "manual",
            ProcessorType::Generator => "generator",
            ProcessorType::DataSource => "datasource",
        }
    }
}

impl Default for ProcessorType {
    fn default() -> Self { ProcessorType::Auto }

}

impl fmt::Display for ProcessorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Default)]
pub struct Processor {
    pub id: String,
    pub name: String,
    pub processortype: ProcessorType,
    pub version: String,
    pub folia_version: String,
    pub document_version: String,
    pub command: String,
    pub host: String,
    pub user: String,
    pub begindatetime: String,
    pub enddatetime: String,
    pub processors: Vec<ProcKey>,
    pub src: String,
    pub format: String,
    pub resourcelink: String,
    pub parent: Option<ProcKey>,
    pub metadata: Metadata,
}

impl CheckEncoded for Processor { }

impl MaybeIdentifiable for Processor {
    fn id(&self) -> Option<String> {
        Some(self.id.clone())
    }
}

#[derive(Default)]
pub struct Metadata {
    pub data: HashMap<String,String>,
    pub src: Option<String>,
    pub metadatatype: Option<String>,
}

