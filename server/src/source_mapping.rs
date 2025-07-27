
use nano_crl2::analysis::semantic::name_resolution::{get_defs_in_context, NameLookup, NameLookupEnum};
use nano_crl2::core::syntax::{Identifier, SourceCursorPos, SourceRange};
use nano_crl2::ir::decl::DefId;
use nano_crl2::ir::expr::IrExprEnum;
use nano_crl2::ir::iterator::{IrIterator, ParentIterator};
use nano_crl2::ir::module::{IrModule, NodeId};
use nano_crl2::ir::proc::IrProcEnum;
use nano_crl2::ir::sort::IrSortEnum;

/// Returns the smallest node that contains the given location, or `None` if
/// there is no node at that location.
/// 
/// This is implemented rather naively right now, since it still searches the
/// entire IR.
pub fn get_node_at_loc(
    module: &IrModule,
    loc: SourceCursorPos,
) -> (SourceRange, NodeId) {
    let mut best = (module.loc, NodeId::Module(module.id));
    let mut best_distance = (u32::MAX, i64::MAX);
    for node in module {
        let node_loc = module.get_node_loc(node);
        if node_loc.contains_cursor(loc) && node_loc.get_distance() <= best_distance {
            best = (node_loc, node);
            best_distance = node_loc.get_distance();
        }
    }
    best
}

/// Returns a 3-tuple that stores an identifier at the given location, or
/// `None` if there is no identifier at that location.
/// 
/// The return value is a 3-tuple of:
/// - The source range of the identifier containing the given location
/// - The node that stores the identifier
/// - A definition ID, where `Some` means that this is a definition (e.g. `x`
/// in `forall x: Nat . y`) and `None` means that it is a name that *refers* to
/// a definition (e.g. `y` in `forall x: Nat . y`)
/// 
/// This is implemented rather naively right now, since it still searches the
/// entire IR until it finds the given location. 
pub fn get_identifier_node_at_loc(
    module: &IrModule,
    loc: SourceCursorPos,
) -> Option<(SourceRange, NodeId, Option<DefId>)> {
    let iterator = IdentifierIterator {
        module,
        ir_iterator: module.into_iter(),
    };
    for (_identifier, identifier_loc, id, is_def) in iterator {
        if identifier_loc.contains_cursor(loc) {
            return Some((identifier_loc, id, is_def));
        }
    }
    None
}

/// Each item is a 4-tuple of:
/// - The identifier that is found in the source
/// - The source location of the identifier (not necessarily of the node!)
/// - The node that the identifier can be found in
/// - A definition ID, where `Some` means that this is a definition (e.g. `x`
/// in `forall x: Nat . y`) and `None` means that it is a name that *refers* to
/// a definition (e.g. `y` in `forall x: Nat . y`)
pub struct IdentifierIterator<'m> {
    module: &'m IrModule,
    ir_iterator: IrIterator<'m>,
}

impl<'m> IdentifierIterator<'m> {
    pub fn new(module: &'m IrModule, starting_node: NodeId) -> Self {
        IdentifierIterator {
            module,
            ir_iterator: IrIterator::new(module, starting_node),
        }
    }
}

impl<'m> Iterator for IdentifierIterator<'m> {
    type Item = (&'m Identifier, SourceRange, NodeId, Option<DefId>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.ir_iterator.next() {
            match item {
                NodeId::Action(id) => {
                    let action = self.module.get_action(id);
                    return Some((
                        &action.identifier,
                        action.identifier_loc,
                        id.into(),
                        None,
                    ));
                },
                NodeId::Decl(id) => {
                    let decl = self.module.get_decl(id);
                    return Some((
                        &decl.identifier,
                        decl.identifier_loc,
                        id.into(),
                        Some(decl.def_id),
                    ));
                },
                NodeId::Expr(id) => {
                    let expr = self.module.get_expr(id);
                    match &expr.value {
                        IrExprEnum::Name { identifier } => {
                            return Some((
                                &identifier,
                                expr.loc,
                                id.into(),
                                None,
                            ));
                        },
                        IrExprEnum::Binder { def_id, identifier, identifier_loc, .. } => {
                            return Some((
                                &identifier,
                                *identifier_loc,
                                id.into(),
                                Some(*def_id),
                            ));
                        },
                        _ => {},
                    }
                },
                NodeId::Module(_) => {},
                NodeId::Param(id) => {
                    let param = self.module.get_param(id);
                    return Some((
                        &param.identifier,
                        param.identifier_loc,
                        id.into(),
                        Some(param.def_id),
                    ));
                },
                NodeId::Proc(id) => {
                    let proc = self.module.get_proc(id);
                    match &proc.value {
                        IrProcEnum::Sum { def_id, identifier, identifier_loc, .. } => {
                            return Some((
                                identifier,
                                *identifier_loc,
                                id.into(),
                                Some(*def_id),
                            ));
                        },
                        _ => {},
                    }
                },
                NodeId::RewriteSet(_) => {},
                NodeId::RewriteRule(_) => {},
                NodeId::RewriteVar(id) => {
                    let rewrite_var = self.module.get_rewrite_var(id);
                    return Some((
                        &rewrite_var.identifier,
                        rewrite_var.identifier_loc,
                        id.into(),
                        Some(rewrite_var.def_id),
                    ));
                },
                NodeId::Sort(id) => {
                    let sort = self.module.get_sort(id);
                    match &sort.value {
                        IrSortEnum::Name { identifier } => {
                            return Some((
                                &identifier,
                                sort.loc,
                                id.into(),
                                None,
                            ));
                        },
                        _ => {},
                    }
                },
            }
        }
        None
    }
}

/// Returns the set of definitions that are valid at the given source location.
pub fn get_def_context_at_loc(
    ir_module: &IrModule,
    loc: SourceCursorPos,
) -> Result<Vec<DefId>, ()> {
    let (_, node_id) = get_node_at_loc(&ir_module, loc);
    let mut result = Vec::new();
    for node in ParentIterator::new(&ir_module, node_id) {
        let def_ids = get_defs_in_context(&ir_module, node, &NameLookup {
            value: NameLookupEnum::All,
            identifier: None,
            loc: ir_module.get_node_loc(node),
        });
        result.extend(def_ids);
    }
    Ok(result)
}
