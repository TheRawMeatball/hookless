mod blinker;

use super::*;
use std::{collections::HashSet, fmt::Display};

use blinker::Blinker;

#[derive(Default)]
struct DemoDom {
    counter: u64,
    roots: HashSet<PrimitiveId>,
    dom: HashMap<PrimitiveId, (Primitive, HashSet<PrimitiveId>)>,
}

impl Display for DemoDom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "The Dom")?;
        writeln!(f, "=======")?;
        fn recursor(
            f: &mut std::fmt::Formatter<'_>,
            element: PrimitiveId,
            nest_level: i32,
            dom: &HashMap<PrimitiveId, (Primitive, HashSet<PrimitiveId>)>,
        ) -> std::fmt::Result {
            for _ in 0..=nest_level {
                write!(f, "|>")?;
            }
            let (primitive, children) = dom.get(&element).unwrap();
            match primitive {
                Primitive::Text(text) => writeln!(f, "{}", text)?,
                Primitive::Panel => writeln!(f, "[Fancy Panel]")?,
            }
            for child in children {
                recursor(f, *child, nest_level + 1, dom)?;
            }
            Ok(())
        }

        for (i, root) in self.roots.iter().enumerate() {
            writeln!(f, "Begin root {}", i + 1)?;
            recursor(f, *root, 0, &self.dom)?;
            writeln!(f, "End root {}", i + 1)?;
        }

        Ok(())
    }
}

impl Dom for DemoDom {
    fn diff_primitive(&mut self, old: PrimitiveId, new: Primitive) {
        self.dom.get_mut(&old).unwrap().0 = new;
    }
    fn remove(&mut self, id: PrimitiveId) {
        self.dom.remove(&id);
        self.roots.remove(&id);
    }

    fn mount_as_child(&mut self, primitive: Primitive, parent: Option<PrimitiveId>) -> PrimitiveId {
        let id = PrimitiveId(self.counter);
        self.counter += 1;
        self.dom.insert(id, (primitive, HashSet::new()));
        if let Some(pid) = parent {
            self.dom.get_mut(&pid).unwrap().1.insert(id);
        } else {
            self.roots.insert(id);
        }
        id
    }

    fn get_sub_list(&mut self, id: PrimitiveId) -> (PrimitiveId, &mut dyn Dom) {
        (id, self)
    }
}

#[test]
fn demo() {
    let mut dom = DemoDom::default();
    println!("{:?}", std::any::TypeId::of::<()>());
    let mut context = Context::new(
        Element::Primitive(Primitive::Panel, vec![Blinker::E(3), Blinker::E(5)]),
        &mut dom,
    );
    loop {
        if context.rx.len() > 0 {
            context.process_messages(&mut dom);
            println!("{}", &dom);
        }
    }
}
