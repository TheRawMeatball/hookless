#[cfg(test)]
mod tests;

use std::{
    any::{type_name, Any, TypeId},
    cell::{Cell, RefCell},
    collections::HashMap,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use append_vec::AppendVec;
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
                println!("Id: {}, Type: {}", id.0, std::any::type_name::<T>());
                let mut c: &mut BuiltComponent = x.get(id);
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
    type Props: Clone + 'static;

    fn render(&self, props: &Self::Props, ctx: Ctx<Self>) -> Vec<Element>;
    fn new(props: &Self::Props, ctx: Ctx<Self>) -> Self;
    fn post_mount(_state: Tracked<Self>, _ctx: Ctx<Self>) {}
    fn post_update(
        _state: Tracked<Self>,
        _old_props: &Self::Props,
        _new_props: &Self::Props,
        _ctx: Ctx<Self>,
    ) {
    }
    fn unmount(self) {}
    fn as_fn_component(&mut self) -> Option<&mut dyn DynFnComponent> {
        None
    }
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
    fn as_fn_component(&mut self) -> Option<&mut dyn DynFnComponent>;
}

trait Prop {
    fn print_type(&self);
    fn dyn_clone(&self) -> Box<dyn Prop>;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Clone> Prop for T {
    fn print_type(&self) {
        println!("{}", type_name::<T>())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_clone(&self) -> Box<dyn Prop> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Prop> {
    fn clone(&self) -> Self {
        (**self).dyn_clone()
    }
}

impl<T: Component> DynComponent for T {
    fn render(&self, props: &dyn Prop, tx: &Tx, id: ComponentId) -> Vec<Element> {
        props.print_type();
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
        println!("Converting &mut {} to &mut dyn Any!", std::any::type_name::<Self>());
        self
    }
    fn as_fn_component(&mut self) -> Option<&mut dyn DynFnComponent> {
        self.as_fn_component()
    }
}

#[derive(Clone)]
pub struct ComponentTemplate {
    new: fn(&dyn Prop, tx: &Tx, id: ComponentId) -> Box<dyn DynComponent>,
    props: Box<dyn Prop>,
    id: TypeId,
}

struct BuiltComponent {
    state: Box<dyn DynComponent>,
    props: Box<dyn Prop>,
    dirty: bool,
}

pub struct Tracked<'a, T: ?Sized>(&'a mut T, &'a mut bool);
impl<'a, T: ?Sized> Tracked<'a, T> {
    pub fn get_untracked(&mut self) -> &mut T {
        self.0
    }

    pub fn set_modified(&mut self) {
        *self.1 = true;
    }
}

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
        Element::Component(ComponentTemplate {
            new: |v, tx, id| {
                Box::new(C::new(
                    v.as_any().downcast_ref::<C::Props>().unwrap(),
                    Ctx {
                        tx: tx.clone(),
                        id,
                        _m: PhantomData,
                    },
                ))
            },
            props: Box::new(props),
            id: TypeId::of::<C>(),
        })
    }
}
#[derive(Clone)]
pub enum Primitive {
    Text(String),
    Panel,
}

#[derive(Clone)]
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
                let id = components.allocate();
                let mut state = (c.new)(&*c.props, tx, id);
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
                if components.get(*bcid).state.type_of() == ct.id {
                    components.with(*bcid, |bc, components| {
                        let mut changed = true;
                        let mut new_props = Some(ct.props);
                        while changed {
                            changed = false;
                            let prop_ref = &**new_props.as_ref().unwrap_or(&bc.props);
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
                                .post_update(&mut changed, &*bc.props, prop_ref, tx, *bcid);

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
                let mut updated_children = false;
                components.with(*id, |c, components| {
                    while c.dirty {
                        updated_children = true;
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

                if !updated_children {
                    for child in children.iter_mut() {
                        child.rerender_flagged(dom, components, tx);
                    }
                }
            }
            MountedElement::Primitive(id, children) => {
                for child in children {
                    let mut dom = dom.get_sub_list(*id);
                    child.rerender_flagged(&mut dom, components, tx);
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

struct FnComponent<P: Prop> {
    states: AppendVec<State>,
    effects: Vec<Effect>,
    _m: PhantomData<fn() -> P>,
}

pub trait DynFnComponent {
    fn get_states(&mut self) -> &mut AppendVec<State>;
}

impl<P: Prop> DynFnComponent for FnComponent<P> {
    fn get_states(&mut self) -> &mut AppendVec<State> {
        &mut self.states
    }
}

impl<P: Prop + Clone + 'static> Component for FnComponent<P> {
    type Props = (Box<dyn ComponentFn<P>>, P);

    fn render(&self, props: &Self::Props, ctx: Ctx<Self>) -> Vec<Element> {
        let mut ro = Vec::new();
        props.0.call(
            &props.1,
            Fctx {
                tx: ctx.tx,
                id: ctx.id,
                states: &self.states,
                effects: None,
                render_output: Cell::new(Some(&mut ro)),
                state_selector: Default::default(),
                effect_selector: Default::default(),
                init: false,
            },
        );
        ro
    }

    fn new(props: &Self::Props, ctx: Ctx<Self>) -> Self {
        let states = AppendVec::new();
        let mut effects = Vec::new();
        props.0.call(
            &props.1,
            Fctx {
                tx: ctx.tx,
                id: ctx.id,
                states: &states,
                effects: Some(RefCell::new(&mut effects)),
                render_output: Cell::new(None),
                state_selector: Default::default(),
                effect_selector: Default::default(),
                init: true,
            },
        );
        Self {
            states,
            effects,
            _m: PhantomData,
        }
    }

    fn post_update(
        mut t_state: Tracked<Self>,
        _old_props: &Self::Props,
        props: &Self::Props,
        ctx: Ctx<Self>,
    ) {
        let state = t_state.get_untracked();
        props.0.call(
            &props.1,
            Fctx {
                tx: ctx.tx.clone(),
                id: ctx.id,
                states: &state.states,
                effects: Some(RefCell::new(&mut state.effects)),
                render_output: Cell::new(None),
                state_selector: Default::default(),
                effect_selector: Default::default(),
                init: false,
            },
        );
    }

    fn post_mount(mut t_state: Tracked<Self>, _: Ctx<Self>) {
        let state = t_state.get_untracked();
        for effect in state.effects.iter_mut() {
            replace_with::replace_with_or_abort(effect, |effect| match effect.f {
                EffectStage::Effect(e) => Effect {
                    eq_cache: effect.eq_cache,
                    f: EffectStage::Destructor(e()),
                },
                EffectStage::Destructor(_) => effect,
            });
        }
    }

    fn unmount(self) {
        for effect in self.effects.into_iter() {
            match effect.f {
                EffectStage::Effect(_) => {}
                EffectStage::Destructor(d) => {
                    d();
                }
            }
        }
    }

    fn as_fn_component(&mut self) -> Option<&mut dyn DynFnComponent> {
        Some(self)
    }
}

pub trait ComponentFn<P>: 'static {
    fn e(&self, props: P) -> Element;
    fn call(&self, props: &P, ctx: Fctx);
    fn fn_clone(&self) -> Box<dyn ComponentFn<P>>;
}

impl<P: 'static> Clone for Box<dyn ComponentFn<P>> {
    fn clone(&self) -> Self {
        self.fn_clone()
    }
}

#[allow(non_snake_case)]
mod impls {
    use super::*;
    macro_rules! impl_component_fn {
        ($($ident: ident),*) => {
            impl<Func, $($ident,)*> ComponentFn<($($ident,)*)> for Func
            where
                $($ident: Clone + 'static,)*
                Func: Fn(Fctx, $(&$ident,)*) + Copy + 'static,
            {
                fn e(&self, props: ($($ident,)*)) -> Element {
                    FnComponent::<($($ident,)*)>::E((Box::new(*self), props))
                }

                fn call(&self, props: &($($ident,)*), ctx: Fctx<'_>) {
                    let ($($ident,)*) = props;
                    self(ctx, $($ident,)*);
                }

                fn fn_clone(&self) -> Box<dyn ComponentFn<($($ident,)*)>> {
                    Box::new(*self)
                }
            }
        };
    }

    impl_component_fn!();
    impl_component_fn!(A);
    impl_component_fn!(A, B);
    impl_component_fn!(A, B, C);
    impl_component_fn!(A, B, C, D);
}

pub struct Fctx<'a> {
    tx: Tx,
    id: ComponentId,
    states: &'a AppendVec<State>,
    effects: Option<RefCell<&'a mut Vec<Effect>>>,
    render_output: Cell<Option<&'a mut Vec<Element>>>,
    state_selector: Cell<usize>,
    effect_selector: Cell<usize>,
    init: bool,
}

pub struct State {
    val: Box<dyn Any>,
}

struct Effect {
    eq_cache: Option<Box<dyn Any>>,
    f: EffectStage,
}

enum EffectStage {
    Effect(Box<dyn FnOnce() -> Box<dyn FnOnce()>>),
    Destructor(Box<dyn FnOnce()>),
}

impl State {
    fn new(val: Box<dyn Any>) -> Self {
        Self { val }
    }
}

pub struct Setter<T> {
    tx: Tx,
    component: ComponentId,
    state: usize,
    _m: PhantomData<fn() -> T>,
}

impl<T> PartialEq for Setter<T> {
    fn eq(&self, other: &Self) -> bool {
        self.component == other.component && self.state == other.state
    }
}

impl<'a> Fctx<'a> {
    pub fn use_state<T: 'static, F: Fn() -> T>(&self, default: F) -> (&T, Setter<T>) {
        let state = if self.init {
            self.states.push(State::new(Box::new(default())));
            let state = self.states.last().unwrap();
            (&*state.val).downcast_ref().unwrap()
        } else {
            let state = self.states.get(self.state_selector.get()).unwrap();
            (&*state.val).downcast_ref().unwrap()
        };
        self.state_selector.set(self.state_selector.get() + 1);
        (
            state,
            Setter {
                tx: self.tx.clone(),
                component: self.id,
                state: self.state_selector.get() - 1,
                _m: PhantomData,
            },
        )
    }

    pub fn use_effect<F, D, X>(&self, eq_cache: Option<X>, f: F)
    where
        F: FnOnce() -> D + Send + Sync + 'static,
        D: FnOnce() + Send + Sync + 'static,
        X: PartialEq + 'static,
    {
        if let Some(effects) = &self.effects {
            let mut effects = effects.borrow_mut();
            if self.init {
                effects.push(Effect {
                    eq_cache: eq_cache.map(|x| Box::new(x) as Box<dyn Any>),
                    f: EffectStage::Effect(Box::new(move || Box::new(f()))),
                });
            } else {
                if effects
                    .get(self.effect_selector.get())
                    .and_then(|v| v.eq_cache.as_ref())
                    .and_then(|v| (&**v).downcast_ref::<X>())
                    .zip(eq_cache.as_ref())
                    .map(|(v, f)| v != f)
                    .unwrap_or(true)
                {
                    let old = effects.get_mut(self.effect_selector.get()).unwrap();
                    replace_with::replace_with_or_abort(old, move |v| {
                        match v.f {
                            EffectStage::Effect(_) => {}
                            EffectStage::Destructor(d) => {
                                d();
                            }
                        }
                        Effect {
                            eq_cache: eq_cache.map(|x| Box::new(x) as Box<dyn Any>),
                            f: EffectStage::Effect(Box::new(move || Box::new(f()))),
                        }
                    });
                }
                self.state_selector.set(self.state_selector.get() + 1);
            }
        }
    }

    /// Only call once!
    pub fn render<F: FnOnce() -> Vec<Element>>(&self, f: F) {
        if let Some(ro) = self.render_output.take() {
            *ro = f();
        }
    }
}

impl<T: 'static> Setter<T> {
    pub fn set<F: FnOnce(&mut T) + Send + Sync + 'static>(&self, f: F) {
        let id = self.component;
        let state = self.state;
        self.tx
            .send(Box::new(move |components| {
                let c: &mut BuiltComponent = components.get(id);
                let states = (&mut *c.state).as_fn_component().unwrap().get_states();
                let state = &mut *states.inner()[state];
                f((&mut *state.val).downcast_mut().unwrap());
                c.dirty = true;
            }))
            .unwrap();
    }
}
