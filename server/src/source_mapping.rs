
use nano_crl2::core::syntax::{Identifier, SourceCursorPos, SourceRange};
use nano_crl2::ir::expr::IrExprEnum;
use nano_crl2::ir::iterator::IrIterator;
use nano_crl2::ir::module::{IrModule, NodeId};
use nano_crl2::ir::proc::IrProcEnum;
use nano_crl2::ir::sort::IrSortEnum;

pub fn get_identifier_node_at_loc(
    module: &IrModule,
    loc: SourceCursorPos,
) -> Option<(SourceRange, NodeId, bool)> {
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
/// - The identifier that is found in the source;
/// - The source location of the identifier (not necessarily of the node!)
/// - The node that the identifier can be found in;
/// - A boolean, where `true` means that this is a definition (e.g. `x` in
/// `forall x: Nat . y`) and `false` means that it is a name that *refers* to a
/// definition (e.g. `y` in `forall x: Nat . y`).
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
    type Item = (&'m Identifier, SourceRange, NodeId, bool);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.ir_iterator.next() {
            match item {
                NodeId::Action(id) => {
                    let action = self.module.get_action(id);
                    return Some((
                        &action.identifier,
                        action.identifier_loc,
                        id.into(),
                        false,
                    ));
                },
                NodeId::Decl(id) => {
                    let decl = self.module.get_decl(id);
                    return Some((
                        &decl.identifier,
                        decl.identifier_loc,
                        id.into(),
                        true,
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
                                false,
                            ));
                        },
                        IrExprEnum::Binder { identifier, identifier_loc, .. } => {
                            return Some((
                                &identifier,
                                *identifier_loc,
                                id.into(),
                                true,
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
                        true,
                    ));
                },
                NodeId::Proc(id) => {
                    let proc = self.module.get_proc(id);
                    match &proc.value {
                        IrProcEnum::Sum { identifier, identifier_loc, .. } => {
                            return Some((
                                identifier,
                                *identifier_loc,
                                id.into(),
                                true,
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
                        true,
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
                                false,
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
