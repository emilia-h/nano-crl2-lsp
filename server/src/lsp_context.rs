
use crate::source_mapping::{get_identifier_node_at_loc, get_def_context_at_loc, IdentifierIterator};
use crate::util::source_range_to_lsp_range;

use nano_crl2::analysis::context::AnalysisContext;
use nano_crl2::analysis::ir_conversion::module::query_ir_module;
use nano_crl2::analysis::parsing::{query_ast_module, query_token_list};
use nano_crl2::analysis::semantic::name_resolution::query_def_of_name;
use nano_crl2::core::lexer::Token;
use nano_crl2::core::syntax::{ModuleId, SourceCursorPos, SourceRange};
use nano_crl2::ir::module::{IrModule, NodeId};
use nano_crl2::ir::iterator::get_def_data;
use nano_crl2::model::module::Module;
use tower_lsp::lsp_types::{CompletionItem, Position, Range};

use std::collections::hash_map::{Entry, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct LspContext(pub Mutex<LspContextStore>);

impl LspContext {
    pub fn new() -> Self {
        LspContext(Mutex::new(LspContextStore {
            analysis_context: AnalysisContext::new(),
            file_inputs: HashMap::new(),
            last_valid_irs: HashMap::new(),
        }))
    }

    /// Changes (or adds if it does not exist already) the model input string
    /// associated with the given file name.
    pub fn set_file(
        &self,
        file_name: String,
        value: String,
    ) -> Result<(), ()> {
        let mut guard = self.lock()?;
        let new_module_id = guard.analysis_context.add_model_input(file_name.clone(), value);

        guard.file_inputs.insert(file_name.clone(), new_module_id);

        Ok(())
    }

    pub fn query_token_list(&self, file_name: &str) -> Result<Arc<Vec<Token>>, ()> {
        let guard = self.lock()?;
        let Some(&module_id) = guard.file_inputs.get(file_name) else {
            return Err(())
        };
        query_token_list(&guard.analysis_context, module_id)
    }

    pub fn query_ast(&self, file_name: &str) -> Result<Arc<Module>, ()> {
        let guard = self.lock()?;
        let Some(&module_id) = guard.file_inputs.get(file_name) else {
            return Err(())
        };
        query_ast_module(&guard.analysis_context, module_id)
    }

    pub fn query_identifier_node_at_loc(
        &self,
        file_name: &str,
        loc: SourceCursorPos,
    ) -> Result<Option<(SourceRange, NodeId, bool)>, ()> {
        // NOTE: maybe it is better to not use last_valid_irs, because we are
        // querying based on a precise location (so it MUST be the version that
        // the user is currently seeing, even if that makes it less tolerant)
        let mut guard = self.lock()?;
        let ir_module = guard.get_last_valid_ir_module(file_name)?;
        drop(guard);
        Ok(get_identifier_node_at_loc(&ir_module, loc))
    }

    pub fn query_completion_items(
        &self,
        file_name: &str,
        loc: SourceCursorPos,
    ) -> Result<Vec<CompletionItem>, ()> {
        let mut guard = self.lock()?;
        let ir_module = guard.get_last_valid_ir_module(file_name)?;
        drop(guard);
        let bla = get_def_context_at_loc(&ir_module, loc)?;
        let result = bla
            .into_iter()
            .map(|def_id| {
                let node = ir_module.get_def_source(def_id);
                let (def_id2, identifier, _) = get_def_data(&ir_module, node).unwrap();
                assert_eq!(def_id, def_id2);
                CompletionItem {
                    label: identifier.to_string(),
                    ..Default::default()
                }
            })
            .collect();
        Ok(result)
    }

    pub fn query_definition(
        &self,
        node_id: NodeId,
    ) -> Result<(SourceRange, SourceRange), ()> {
        let guard = self.lock()?;
        let def_id = query_def_of_name(&guard.analysis_context, node_id)?;
        let ir_module = query_ir_module(
            &guard.analysis_context,
            def_id.get_module_id(),
        )?;
        drop(guard);
        let node_id = ir_module.get_def_source(def_id);
        let identifier_loc = get_def_data(&ir_module, node_id).unwrap().2;
        let node_loc = ir_module.get_node_loc(node_id);
        Ok((identifier_loc, node_loc))
    }

    pub fn query_references(
        &self,
        source_id: NodeId,
    ) -> Result<Vec<SourceRange>, ()> {
        let guard = self.lock()?;
        let ir_module = query_ir_module(
            &guard.analysis_context,
            source_id.get_module_id(),
        )?;

        let source_identifier = get_def_data(&ir_module, source_id).unwrap().1;

        let start_node = if matches!(source_id, NodeId::Decl(..) | NodeId::Param(..) | NodeId::RewriteVar(..)) {
            let Some(parent) = ir_module.get_parent(source_id) else {
                return Ok(Vec::new())
            };
            parent
        } else {
            source_id
        };
        let iterator = IdentifierIterator::new(&ir_module, start_node);

        let mut result = Vec::new();
        for (identifier, loc, target, is_def) in iterator {
            if is_def || identifier != source_identifier {
                continue;
            }
            let def = query_def_of_name(&guard.analysis_context, target)?;
            let def_source = ir_module.get_def_source(def);
            if def_source == source_id {
                result.push(loc);
            }
        }
        Ok(result)
    }

    pub fn get_diagnostics(
        &self,
        file_name: &str,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let guard = match self.lock() {
            Ok(value) => value,
            Err(()) => {
                return vec![tower_lsp::lsp_types::Diagnostic::new(
                    Range::new(Position::new(0, 0), Position::new(0, 0)),
                    Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
                    None, // code
                    Some("nanoCRL2-lsp".to_owned()),
                    "mysterious multithreading error".to_owned(),
                    None, // related information
                    None, // tags
                )];
            },
        };
        let Some(&module) = guard.file_inputs.get(file_name) else {
            return Vec::new()
        };

        let mut result = Vec::new();
        guard.analysis_context.for_each_diagnostic(|diagnostic| {
            if diagnostic.module != Some(module) {
                return;
            }
            let severity = match diagnostic.severity {
                nano_crl2::core::diagnostic::DiagnosticSeverity::Error =>
                    tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
                nano_crl2::core::diagnostic::DiagnosticSeverity::Warning =>
                    tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
                nano_crl2::core::diagnostic::DiagnosticSeverity::Message =>
                    tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION,
            };
            let range = if let Some(loc) = diagnostic.loc {
                source_range_to_lsp_range(loc)
            } else {
                Range::new(Position::new(0, 0), Position::new(0, 0))
            };
            result.push(tower_lsp::lsp_types::Diagnostic::new(
                range,
                Some(severity),
                None, // error code
                Some("nanoCRL2-lsp".to_owned()),
                diagnostic.message.clone(),
                None, // related information
                None, // tag
            ));
        });
        result
    }

    fn lock(&self) -> Result<MutexGuard<LspContextStore>, ()> {
        self.0.lock().map_err(|_error| {
            // TODO report error somewhere
            ()
        })
    }
}

pub struct LspContextStore {
    analysis_context: AnalysisContext,
    file_inputs: HashMap<String, ModuleId>,
    /// Since a user is constantly typing and modifying the file string, it is
    /// often useful to have some kind of reference to the last valid IR, so we
    /// can still extract semantic information.
    pub last_valid_irs: HashMap<String, ModuleId>,
}

impl LspContextStore {
    pub fn get_last_valid_ir_module(&mut self, file_name: &str) -> Result<Arc<IrModule>, ()> {
        let Some(&module_id) = self.file_inputs.get(file_name) else {
            return Err(())
        };
        match query_ir_module(&self.analysis_context, module_id) {
            Ok(ir_module) => {
                // if the newest version has a valid IR, update the
                // `last_valid_irs` entry (deleting the old if necessary)
                match self.last_valid_irs.entry(file_name.to_owned()) {
                    Entry::Occupied(mut entry) => {
                        if *entry.get() != module_id {
                            self.analysis_context.remove_model_input(*entry.get());
                            *entry.get_mut() = module_id;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(module_id);
                    },
                }
                Ok(ir_module)
            },
            Err(()) => {
                // if the newest version does not have a valid IR, try to fall
                // back to the last valid IR
                match self.last_valid_irs.get(file_name) {
                    Some(&old_ir_module_id) => {
                        query_ir_module(&self.analysis_context, old_ir_module_id)
                    },
                    None => {
                        Err(())
                    },
                }
            },
        }
    }
}
