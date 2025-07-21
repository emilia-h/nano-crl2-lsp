
use crate::source_mapping::{get_identifier_node_at_loc, IdentifierIterator};
use crate::util::source_range_to_lsp_range;

use nano_crl2::analysis::context::AnalysisContext;
use nano_crl2::analysis::ir_conversion::module::query_ir_module;
use nano_crl2::analysis::parsing::{query_ast_module, query_token_list};
use nano_crl2::analysis::semantic::name_resolution::query_def_of_name;
use nano_crl2::core::lexer::Token;
use nano_crl2::core::syntax::{SourceCursorPos, SourceRange};
use nano_crl2::ir::module::{ModuleId, NodeId};
use nano_crl2::ir::iterator::{get_def_data, get_node_loc};
use nano_crl2::model::module::Module;
use tower_lsp::lsp_types::{Position, Range};

use std::collections::hash_map::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct LspContext(Mutex<LspContextStore>);

impl LspContext {
    pub fn new() -> Self {
        LspContext(Mutex::new(LspContextStore {
            analysis_context: AnalysisContext::new(),
            file_inputs: HashMap::new(),
        }))
    }

    pub fn set_file(
        &self,
        file_name: String,
        value: String,
    ) -> Result<(), ()> {
        let mut guard = self.lock()?;
        if let Some(&old_module_id) = guard.file_inputs.get(&file_name) {
            guard.analysis_context.remove_model_input(old_module_id);
            let new_module_id = guard.analysis_context.add_model_input(file_name.clone(), value);
            guard.file_inputs.insert(file_name, new_module_id).unwrap();
        } else {
            let module_id = guard.analysis_context.add_model_input(file_name.clone(), value);
            guard.file_inputs.insert(file_name, module_id);
        }

        Ok(())
    }

    pub fn query_token_list(&self, file_name: &str) -> Result<Arc<Vec<Token>>, ()> {
        let guard = self.lock()?;
        let module_id = match guard.file_inputs.get(file_name) {
            Some(&value) => value,
            None => return Err(()),
        };
        query_token_list(&guard.analysis_context, module_id)
    }

    pub fn query_ast(&self, file_name: &str) -> Result<Arc<Module>, ()> {
        let guard = self.lock()?;
        let module_id = match guard.file_inputs.get(file_name) {
            Some(&value) => value,
            None => return Err(()),
        };
        query_ast_module(&guard.analysis_context, module_id)
    }

    pub fn query_identifier_node_at_loc(
        &self,
        file_name: &str,
        loc: SourceCursorPos,
    ) -> Result<Option<(SourceRange, NodeId, bool)>, ()> {
        // we don't cache this query (I am lazy, and probably not worth it)
        let guard = self.lock()?;
        let module_id = match guard.file_inputs.get(file_name) {
            Some(&value) => value,
            None => return Err(()),
        };
        let ir_module = query_ir_module(&guard.analysis_context, module_id)?;
        Ok(get_identifier_node_at_loc(&ir_module, loc))
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
        let node_id = ir_module.get_def_source(def_id);
        let identifier_loc = get_def_data(&ir_module, node_id).unwrap().2;
        let node_loc = get_node_loc(&ir_module, node_id);
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
                return Ok(Vec::new());
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

    pub fn get_diagnostics(&self) -> Vec<tower_lsp::lsp_types::Diagnostic> {
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

        let mut result = Vec::new();
        guard.analysis_context.for_each_diagnostic(|diagnostic| {
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
                None, // code
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

struct LspContextStore {
    analysis_context: AnalysisContext,
    file_inputs: HashMap<String, ModuleId>,
}
