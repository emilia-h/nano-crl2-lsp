
use nano_crl2::analysis::context::AnalysisContext;
use nano_crl2::analysis::semantic::sort_resolution::{
    get_decl_sort, query_resolved_sort,
};
use nano_crl2::core::syntax::Identifier;
use nano_crl2::ir::decl::{DefId, IrDecl, IrDeclEnum};
use nano_crl2::ir::display::ResolvedSortDisplay;
use nano_crl2::ir::iterator::{DefiningNode, get_defining_node_from_def};
use nano_crl2::ir::module::IrModule;
use nano_crl2::ir::sort::ResolvedSort;
use nano_crl2::util::caching::Interned;

use std::fmt::{Display, Formatter};

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails,
};

pub fn get_completion_item(
    context: &AnalysisContext,
    module: &IrModule,
    def: DefId,
) -> CompletionItem {
    let def_info = get_def_info(context, module, def);

    let resolved_sort = def_info.sort.as_ref()
        .map(|x| ResolvedSortDisplay::new(module, &x).to_string());
    let label_detail = resolved_sort.as_ref().map(|x| format!(": {}", x));

    let detail = DefInfoDisplay::new(module, &def_info).to_string();

    CompletionItem {
        label: def_info.identifier.to_string(),
        label_details: Some(CompletionItemLabelDetails {
            detail: label_detail,
            description: None,
        }),
        kind: Some(def_info.completion_item_kind),
        detail: Some(detail),
        // TODO documentation: Some(...)
        ..Default::default()
    }
}

pub fn get_def_info<'a>(
    context: &AnalysisContext,
    module: &'a IrModule,
    def: DefId,
) -> DefInfo<'a> {
    use DefiningNode::*;

    assert_eq!(module.id, def.get_module_id());
    match get_defining_node_from_def(module, def) {
        Decl(decl) => {
            let sort = get_decl_sort(decl)
                .and_then(|sort| query_resolved_sort(context, sort).ok());
            DefInfo {
                identifier: &decl.identifier,
                keyword: Some(decl.value.get_keyword_string()),
                completion_item_kind: decl_to_completion_item_kind(decl),
                sort,
            }
        },
        BinderExpr { identifier, sort, .. } => {
            DefInfo {
                identifier,
                keyword: None,
                completion_item_kind: CompletionItemKind::VARIABLE,
                sort: query_resolved_sort(context, sort).ok(),
            }
        },
        Param(param) => {
            DefInfo {
                identifier: &param.identifier,
                keyword: None,
                completion_item_kind: CompletionItemKind::VARIABLE,
                sort: query_resolved_sort(context, param.sort).ok(),
            }
        },
        SumProc { identifier, sort, .. } => {
            DefInfo {
                identifier,
                keyword: None,
                completion_item_kind: CompletionItemKind::VARIABLE,
                sort: query_resolved_sort(context, sort).ok(),
            }
        },
        RewriteVar(rewrite_var) => {
            DefInfo {
                identifier: &rewrite_var.identifier,
                keyword: Some("var"),
                completion_item_kind: CompletionItemKind::VARIABLE,
                sort: query_resolved_sort(context, rewrite_var.sort).ok(),
            }
        },
    }
}

pub struct DefInfo<'a> {
    identifier: &'a Identifier,
    keyword: Option<&'static str>,
    completion_item_kind: CompletionItemKind,
    sort: Option<Interned<ResolvedSort>>,
}

pub struct DefInfoDisplay<'a, 'b> {
    module: &'b IrModule,
    def_info: &'b DefInfo<'a>,
}

impl<'a, 'b> DefInfoDisplay<'a, 'b> {
    pub fn new(module: &'b IrModule, def_info: &'b DefInfo<'a>) -> Self {
        DefInfoDisplay { module, def_info }
    }
}

impl<'a, 'b> Display for DefInfoDisplay<'a, 'b> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        if let Some(keyword) = &self.def_info.keyword {
            write!(f, "{} ", keyword)?;
        }
        write!(f, "{}", self.def_info.identifier)?;
        if let Some(s) = &self.def_info.sort {
            write!(f, ": {}", ResolvedSortDisplay::new(self.module, s))?;
        }
        Ok(())
    }
}

fn decl_to_completion_item_kind(decl: &IrDecl) -> CompletionItemKind {
    match &decl.value {
        IrDeclEnum::Action { .. } => CompletionItemKind::EVENT,
        IrDeclEnum::Constructor { .. } => CompletionItemKind::CONSTRUCTOR,
        IrDeclEnum::GlobalVariable { .. } => CompletionItemKind::CONSTANT,
        IrDeclEnum::Map { .. } => CompletionItemKind::FUNCTION,
        IrDeclEnum::Process { .. } => CompletionItemKind::FUNCTION,
        IrDeclEnum::Sort => CompletionItemKind::STRUCT,
        IrDeclEnum::SortAlias { .. } => CompletionItemKind::STRUCT,
    }
}
