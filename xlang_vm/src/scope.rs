use std::collections::HashMap;

use linked_hash_map::LinkedHashMap;
use xlang_core::{
    ast::Expression,
    token::{Operator, SpannedToken, Token},
};
use xlang_util::{
    format::{BoxedGrouper, BoxedGrouperIter, NodeDisplay, TreeDisplay},
    Rf,
};

use crate::const_value::{ConstValue, ConstValueKind, Type};

#[derive(Clone)]
pub enum ScopeValue {
    ConstValue(ConstValue),
    Record {
        ident: String,
        members: LinkedHashMap<String, Type>,
    },
    Module,
}

impl NodeDisplay for ScopeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ScopeValue::ConstValue(ConstValue {
                ty: Type::Function { .. },
                ..
            }) => f.write_str("Function"),
            ScopeValue::ConstValue(_) => f.write_str("Constant Value"),
            ScopeValue::Record { .. } => f.write_str("Record"),
            ScopeValue::Module => f.write_str("Module"),
        }
    }
}

impl TreeDisplay for ScopeValue {
    fn num_children(&self) -> usize {
        match self {
            ScopeValue::ConstValue(c) => c.num_children(),
            ScopeValue::Record { .. } => 1,
            ScopeValue::Module => 0,
        }
    }

    fn child_at(&self, index: usize) -> Option<&dyn TreeDisplay<()>> {
        match self {
            ScopeValue::ConstValue(c) => c.child_at(index),
            ScopeValue::Record { members, .. } => Some(members),
            ScopeValue::Module => None,
        }
    }
}

pub struct Scope {
    pub value: ScopeValue,
    pub children: HashMap<String, Rf<Scope>>,
}

impl Scope {
    pub fn new(value: ScopeValue) -> Scope {
        Scope {
            value,
            children: HashMap::new(),
        }
    }
}

impl NodeDisplay for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("Scope")
    }
}

impl TreeDisplay for Scope {
    fn num_children(&self) -> usize {
        if self.children.len() > 0 {
            2
        } else {
            1
        }
    }

    fn child_at(&self, _index: usize) -> Option<&dyn TreeDisplay<()>> {
        match _index {
            0 => Some(&self.value),
            _ => None,
        }
    }

    fn child_at_bx<'a>(&'a self, _index: usize) -> Box<dyn TreeDisplay<()> + 'a> {
        Box::new(BoxedGrouperIter(
            "Children".to_string(),
            self.children.len(),
            self.children.iter().map(|f| {
                Box::new(BoxedGrouper(f.0.clone(), Box::new(f.1.borrow()))) as Box<dyn TreeDisplay>
            }),
        ))
    }
}

#[derive(Clone)]
pub struct ScopeRef(usize, String);

pub struct ScopeManager {
    module: Rf<Scope>,
    current_scope: Vec<Rf<Scope>>,
}

impl<'a> ScopeManager {
    pub fn new(module: Rf<Scope>) -> ScopeManager {
        let mut vec = Vec::with_capacity(20);
        vec.push(module.clone());

        ScopeManager {
            module,
            current_scope: vec,
        }
    }

    pub fn push_scope(&mut self, rf: Rf<Scope>) {
        self.current_scope.push(rf);
        // self.scopes.push(Scope {
        //     symbols: HashMap::new(),
        // });
    }

    pub fn pop_scope(&mut self) -> Rf<Scope> {
        self.current_scope.remove(self.current_scope.len() - 1)
    }

    // pub fn get_symbol_ref(&self, name: &str) -> Option<ScopeRef> {
    //     let found = self
    //         .scopes
    //         .iter()
    //         .enumerate()
    //         .rev()
    //         .find_map(|scope| scope.1.symbols.get(name).map(|sc| (scope.0, name)));

    //     if let Some((ind, st)) = found {
    //         Some(ScopeRef(ind, st.to_string()))
    //     } else {
    //         None
    //     }
    // }

    // pub fn get_symbol(&self, scope_ref: &ScopeRef) -> Option<&ScopeValue> {
    //     if let Some(scp) = self.scopes.get(scope_ref.0) {
    //         if let Some(sym) = scp.symbols.get(&scope_ref.1) {
    //             return Some(sym);
    //         }
    //     }

    //     None
    // }

    pub fn fom(
        &'a mut self,
        left: &Expression,
        right: &Expression,
        mut cb: impl FnMut(&mut ConstValue),
    ) -> bool {
        match (left, right) {
            (Expression::Ident(left), Expression::Ident(right)) => {
                let Some(sym) = self.find_symbol(left.as_str()) else {
                    return false;
                };
                let mut sym = sym.borrow_mut();
                let ScopeValue::ConstValue(
                        ConstValue {
                            ty: Type::RecordInstance { .. },
                            kind: ConstValueKind::RecordInstance { members }
                        }
                    ) = &mut sym.value else {
                        return false
                    };

                if let Some(m) = members.get_mut(right.as_str()) {
                    cb(m);
                }
                return true;
            }
            _ => (),
        }

        false
    }

    pub fn follow_member_access_mut(
        &'a mut self,
        left: &Expression,
        right: &Expression,
        mut cb: impl FnMut(&mut ConstValue),
    ) -> bool {
        match (left, right) {
            (Expression::Ident(left), Expression::Ident(right)) => {
                let Some(sym) = self.find_symbol(left.as_str()) else {
                    return false;
                };
                let mut sym = sym.borrow_mut();
                let ScopeValue::ConstValue(
                        ConstValue {
                            ty: Type::RecordInstance { .. },
                            kind: ConstValueKind::RecordInstance { members }
                        }
                    ) = &mut sym.value else {
                        return false
                    };

                if let Some(m) = members.get_mut(right.as_str()) {
                    cb(m);
                }
                return true;
            }
            (
                Expression::BinaryExpression {
                    op_token: Some(SpannedToken(_, Token::Operator(Operator::Dot))),
                    left: Some(left),
                    right: Some(right),
                },
                Expression::Ident(member_right),
            ) => {
                return self.fom(left, right, |cv| {
                    let ConstValue {
                        ty: Type::RecordInstance { .. },
                        kind: ConstValueKind::RecordInstance { members }
                    } = cv else {
                        return;
                    };

                    if let Some(m) = members.get_mut(member_right.as_str()) {
                        cb(m);
                    }
                });
            }
            _ => (),
        }

        false
    }

    pub fn find_symbol(&'a self, name: &str) -> Option<Rf<Scope>> {
        self.current_scope
            .iter()
            .rev()
            .find_map(|scope| scope.borrow().children.get(name).cloned())
    }

    // pub fn find_symbol_mut(&mut self, name: &str) -> Option<&mut ScopeValue> {
    //     self.current_scope
    //         .iter_mut()
    //         .rev()
    //         .find_map(|scope| scope.borrow_mut().children.get_mut(name))
    // }

    pub fn update_value(&mut self, name: &str, value: ScopeValue) -> Option<ScopeValue> {
        if let Some(sym) = self.find_symbol(name) {
            let old_value = std::mem::replace(&mut sym.borrow_mut().value, value);
            return Some(old_value);
        }

        if let Some(scp) = self.current_scope.last() {
            scp.borrow_mut()
                .children
                .insert(name.to_string(), Rf::new(Scope::new(value)));
        }

        None
    }

    pub fn insert_value(&mut self, name: &str, value: ScopeValue) -> Rf<Scope> {
        if let Some(scp) = self.current_scope.last() {
            let rf = Rf::new(Scope::new(value));
            scp.borrow_mut()
                .children
                .insert(name.to_string(), rf.clone());
            return rf;
        }
        panic!()
    }
}

impl NodeDisplay for ScopeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("Scope Manager")
    }
}

impl TreeDisplay for ScopeManager {
    fn num_children(&self) -> usize {
        1
    }

    fn child_at(&self, _index: usize) -> Option<&dyn TreeDisplay<()>> {
        None
    }

    fn child_at_bx<'a>(&'a self, _index: usize) -> Box<dyn TreeDisplay<()> + 'a> {
        match _index {
            0 => Box::new(self.module.borrow()),
            1 => Box::new(BoxedGrouperIter(
                "Curent Scope".to_string(),
                self.current_scope.len(),
                self.current_scope
                    .iter()
                    .map(|f| Box::new(f.borrow()) as Box<dyn TreeDisplay>),
            )),
            _ => panic!(),
        }
    }
}
