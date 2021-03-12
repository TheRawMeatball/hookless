#[cfg(test)]
mod tests;

use std::{
    any::{type_name, Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crossbeam_channel::{Receiver, Sender};

type Tx = Sender<Box<dyn FnOnce(&mut Components) + Send>>;
type Rx = Receiver<Box<dyn FnOnce(&mut Components) + Send>>;

pub struct Ctx<T> {
    tx: Tx,
    id: ComponentId,
    _m: PhantomData<fn() -> T>,
}

impl<T> Clone for Ctx<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            id: self.id,
            _m: PhantomData,
        }
    }
}

impl<T: 'static> Ctx<T> {
    pub fn mutate_state<F: FnOnce(&mut T) + Send + 'static>(&self, f: F) {
        let id = self.id;
        self.tx
            .send(Box::new(move |x| {
                let mut c = x.get(id);
                f(c.state.as_any_mut().downcast_mut::<T>().unwrap());
                c.dirty = true;
            }))
            .unwrap();
    }

    fn new(tx: Tx, id: ComponentId) -> Self {
        Ctx {
            tx,
            id,
            _m: PhantomData,
        }
    }
}

pub trait Component: Sized + 'static {
    const E: fn(Self::Props) -> Element = |p| Context::create_element::<Self>(p);
    type Props: 'static;

    fn render(&self, props: &Self::Props, ctx: Ctx<Self>) -> Vec<Element>;
    fn new(props: &Self::Props) -> Self;
    fn post_mount(_state: Tracked<Self>, _ctx: Ctx<Self>) {}
    fn post_update(
        _state: Tracked<Self>,
        _old_props: &Self::Props,
        _new_props: &Self::Props,
        _ctx: Ctx<Self>,
    ) {
    }
    fn unmount(self) {}
}

trait DynComponent {
    fn render(&self, props: &dyn Prop, tx: &Tx, id: ComponentId) -> Vec<Element>;
    fn post_mount(&mut self, track: &mut bool, tx: &Tx, id: ComponentId);
    fn post_update(
        &mut self,
        track: &mut bool,
        old_props: &dyn Prop,
        new_props: &dyn Prop,
        tx: &Tx,
        id: ComponentId,
    );
    fn unmount(self: Box<Self>);
    fn type_of(&self) -> TypeId;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

trait Prop {
    fn print_type(&self);
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> Prop for T {
    fn print_type(&self) {
        println!("{}", type_name::<T>())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T: Component> DynComponent for T {
    fn render(&self, props: &dyn Prop, tx: &Tx, id: ComponentId) -> Vec<Element> {
        self.render(
            props.as_any().downcast_ref::<T::Props>().unwrap(),
            Ctx::new(tx.clone(), id),
        )
    }
    fn post_mount(&mut self, track: &mut bool, tx: &Tx, id: ComponentId) {
        Self::post_mount(Tracked(self, track), Ctx::new(tx.clone(), id))
    }
    fn post_update(
        &mut self,
        track: &mut bool,
        old_props: &dyn Prop,
        new_props: &dyn Prop,
        tx: &Tx,
        id: ComponentId,
    ) {
        Self::post_update(
            Tracked(self, track),
            old_props.as_any().downcast_ref::<T::Props>().unwrap(),
            new_props.as_any().downcast_ref::<T::Props>().unwrap(),
            Ctx::new(tx.clone(), id),
        )
    }
    fn unmount(self: Box<Self>) {
        Component::unmount(*self)
    }
    fn type_of(&self) -> TypeId {
        TypeId::of::<Self>()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct ComponentTemplate {
    inner: fn(&dyn Prop) -> Box<dyn DynComponent>,
    props: Box<dyn Prop>,
}

struct BuiltComponent {
    state: Box<dyn DynComponent>,
    props: Box<dyn Prop>,
    dirty: bool,
}

pub struct Tracked<'a, T: ?Sized>(&'a mut T, &'a mut bool);

impl<'a, T> Deref for Tracked<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}

impl<'a, T> DerefMut for Tracked<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        *self.1 = true;
        self.0
    }
}

struct Components {
    counter: u64,
    inner: HashMap<ComponentId, BuiltComponent>,
}

impl Components {
    fn new() -> Self {
        Self {
            inner: HashMap::new(),
            counter: 0,
        }
    }

    fn get(&mut self, id: ComponentId) -> &mut BuiltComponent {
        self.inner.get_mut(&id).unwrap()
    }

    fn take(&mut self, id: ComponentId) -> BuiltComponent {
        self.inner.remove(&id).unwrap()
    }

    fn with<F: FnOnce(&mut BuiltComponent, &mut Self)>(&mut self, id: ComponentId, f: F) {
        let mut val = self.take(id);
        f(&mut val, self);
        self.inner.insert(id, val);
    }

    fn allocate(&mut self) -> ComponentId {
        let id = ComponentId(self.counter);
        self.counter += 1;
        id
    }

    fn fill(&mut self, id: ComponentId, component: BuiltComponent) {
        self.inner.insert(id, component);
    }
}

pub struct Context {
    root: MountedElement,
    components: Components,
    tx: Tx,
    rx: Rx,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PrimitiveId(pub u64);

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct ComponentId(u64);

pub trait Dom {
    fn mount(&mut self, primitive: Primitive) -> PrimitiveId {
        self.mount_as_child(primitive, None)
    }
    fn mount_as_child(&mut self, primitive: Primitive, parent: Option<PrimitiveId>) -> PrimitiveId;
    fn diff_primitive(&mut self, old: PrimitiveId, new: Primitive);
    fn get_sub_list(&mut self, id: PrimitiveId) -> (PrimitiveId, &mut dyn Dom);
    fn remove(&mut self, id: PrimitiveId);
}

impl<T: Dom + ?Sized> Dom for (PrimitiveId, &mut T) {
    fn mount(&mut self, primitive: Primitive) -> PrimitiveId {
        self.1.mount_as_child(primitive, Some(self.0))
    }

    fn diff_primitive(&mut self, old: PrimitiveId, new: Primitive) {
        self.1.diff_primitive(old, new)
    }

    fn get_sub_list(&mut self, id: PrimitiveId) -> (PrimitiveId, &mut dyn Dom) {
        (id, self)
    }

    fn remove(&mut self, id: PrimitiveId) {
        self.1.remove(id)
    }

    fn mount_as_child(&mut self, primitive: Primitive, parent: Option<PrimitiveId>) -> PrimitiveId {
        self.1.mount_as_child(primitive, parent)
    }
}

impl Context {
    pub fn new(element: Element, dom: &mut impl Dom) -> Self {
        let mut components = Components::new();
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            root: MountedElement::mount(element, dom, &mut components, &tx),
            components,
            tx,
            rx,
        }
    }
    pub fn process_messages(&mut self, dom: &mut impl Dom) {
        for f in self.rx.try_iter() {
            f(&mut self.components);
        }
        self.root
            .rerender_flagged(dom, &mut self.components, &self.tx);
    }

    pub fn create_element<C: Component>(props: C::Props) -> Element {
        println!("{:?}", std::any::TypeId::of::<C::Props>());
        Element::Component(ComponentTemplate {
            inner: |v| Box::new(C::new(v.as_any().downcast_ref::<C::Props>().unwrap())),
            props: Box::new(props),
        })
    }
}

pub enum Primitive {
    Text(String),
    Panel,
}

pub enum Element {
    Primitive(Primitive, Vec<Element>),
    Component(ComponentTemplate),
}

enum MountedElement {
    Primitive(PrimitiveId, Vec<MountedElement>),
    Component(ComponentId, Vec<MountedElement>),
}

impl MountedElement {
    fn mount(inner: Element, dom: &mut impl Dom, components: &mut Components, tx: &Tx) -> Self {
        match inner {
            Element::Primitive(p, c) => {
                let id = dom.mount(p);
                let mut child_ctx = dom.get_sub_list(id);
                let children = c
                    .into_iter()
                    .map(|v| Self::mount(v, &mut child_ctx, components, tx))
                    .collect();
                Self::Primitive(id, children)
            }
            Element::Component(c) => {
                let mut state = (c.inner)(&*c.props);
                let id = components.allocate();
                let mut children: Vec<_> = state
                    .render(&*c.props, &tx, id)
                    .into_iter()
                    .map(|c| Self::mount(c, dom, components, tx))
                    .collect();

                let mut changed = false;
                state.post_mount(&mut changed, tx, id);
                while changed {
                    changed = false;
                    let mut new_children = state.render(&*c.props, tx, id).into_iter();
                    for child in children.iter_mut() {
                        if let Some(new_child) = new_children.next() {
                            child.diff(new_child, dom, components, tx);
                        }
                    }
                    state.post_update(&mut changed, &*c.props, &*c.props, tx, id);
                }
                components.fill(
                    id,
                    BuiltComponent {
                        state,
                        props: c.props,
                        dirty: false,
                    },
                );
                Self::Component(id, children)
            }
        }
    }

    fn diff(&mut self, other: Element, dom: &mut impl Dom, components: &mut Components, tx: &Tx) {
        let mut this = self;
        match (&mut this, other) {
            (MountedElement::Primitive(id, children), Element::Primitive(new, new_children)) => {
                dom.diff_primitive(*id, new);
                let mut remove_index = -1isize;
                let mut dom = dom.get_sub_list(*id);
                let mut new_children = new_children.into_iter();
                for (i, child) in children.iter_mut().enumerate() {
                    if let Some(new_child) = new_children.next() {
                        child.diff(new_child, &mut dom, components, tx);
                        remove_index = i as isize;
                    }
                }
                for child in children.drain((remove_index + 1) as usize..) {
                    child.unmount(&mut dom, components);
                }
                for remaining in new_children {
                    children.push(Self::mount(remaining, &mut dom, components, tx));
                }
            }
            (MountedElement::Component(bcid, children), Element::Component(ct)) => {
                if components.get(*bcid).state.type_id() == ct.type_id() {
                    components.with(*bcid, |bc, components| {
                        let mut changed = true;
                        let mut new_props = Some(ct.props);
                        while changed {
                            changed = false;
                            let prop_ref = new_props.as_ref().unwrap_or(&bc.props);
                            let mut new_children = bc.state.render(prop_ref, tx, *bcid).into_iter();
                            let mut remove_index = -1isize;
                            for (i, child) in children.iter_mut().enumerate() {
                                if let Some(new_child) = new_children.next() {
                                    child.diff(new_child, dom, components, tx);
                                    remove_index = i as isize;
                                }
                            }
                            for child in children.drain((remove_index + 1) as usize..) {
                                child.unmount(dom, components);
                            }
                            for remaining in new_children {
                                children.push(Self::mount(remaining, dom, components, tx));
                            }
                            bc.state
                                .post_update(&mut changed, &*bc.props, &*prop_ref, tx, *bcid);

                            if let Some(np) = new_props.take() {
                                bc.props = np;
                            }
                        }
                    });
                } else {
                    for child in children.drain(..) {
                        child.unmount(dom, components);
                    }
                    replace_with::replace_with(this, Self::broken, |v| {
                        v.unmount(dom, components);
                        Self::mount(Element::Component(ct), dom, components, tx)
                    });
                }
            }
            (_, new) => {
                replace_with::replace_with(this, Self::broken, |v| {
                    v.unmount(dom, components);
                    Self::mount(new, dom, components, tx)
                });
            }
        }
    }

    fn rerender_flagged(&mut self, dom: &mut impl Dom, components: &mut Components, tx: &Tx) {
        match self {
            MountedElement::Component(id, children) => {
                components.with(*id, |c, components| {
                    while c.dirty {
                        c.dirty = false;
                        let mut new_children = c.state.render(&*c.props, tx, *id).into_iter();
                        let mut remove_index = -1isize;
                        for (i, child) in children.iter_mut().enumerate() {
                            if let Some(new_child) = new_children.next() {
                                child.diff(new_child, dom, components, tx);
                                remove_index = i as isize;
                            }
                        }
                        for child in children.drain((remove_index + 1) as usize..) {
                            child.unmount(dom, components);
                        }
                        for remaining in new_children {
                            children.push(Self::mount(remaining, dom, components, tx));
                        }
                        c.state
                            .post_update(&mut c.dirty, &*c.props, &*c.props, tx, *id);
                    }
                });
            }
            MountedElement::Primitive(_, children) => {
                for child in children {
                    child.rerender_flagged(dom, components, tx);
                }
            }
        }
    }

    fn unmount(self, dom: &mut impl Dom, components: &mut Components) {
        match self {
            MountedElement::Primitive(id, children) => {
                for child in children.into_iter() {
                    child.unmount(dom, components)
                }
                dom.remove(id);
            }
            MountedElement::Component(c, children) => {
                for child in children.into_iter() {
                    child.unmount(dom, components);
                }
                let c = components.take(c);
                c.state.unmount();
            }
        }
    }

    fn broken() -> Self {
        MountedElement::Primitive(PrimitiveId(0), vec![])
    }
}

