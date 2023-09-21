use std::{collections::HashMap, future::Future};

use async_trait::async_trait;
use futures_concurrency::prelude::*;

pub trait Plugin<HookContainer> {
    fn apply(&self, hook_container: &mut HookContainer);
}

pub trait Hook {
    type Input: Send + Sync;
    type Output;
}

pub trait SyncBail<H: Hook> {
    fn run(&self, input: &mut H::Input) -> Option<H::Output>;
    fn stage(&self) -> i32 {
        0
    }
}

impl<H: Hook, T: Fn(&mut H::Input) -> Option<H::Output>> SyncBail<H> for T {
    fn run(&self, input: &mut <H as Hook>::Input) -> Option<H::Output> {
        self(input)
    }
}

impl<H: Hook, T: Fn(&mut H::Input) -> Option<H::Output>> SyncBail<H> for (T, i32) {
    fn run(&self, input: &mut <H as Hook>::Input) -> Option<H::Output> {
        self.0(input)
    }
    fn stage(&self) -> i32 {
        self.1
    }
}

pub struct SyncBailHook<H>(Vec<Box<dyn SyncBail<H>>>);

impl<H> Default for SyncBailHook<H> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<H: Hook> SyncBailHook<H> {
    pub fn call(&mut self, input: &mut H::Input) -> Option<H::Output> {
        self.0.sort_by_key(|hook| hook.stage());
        for hook in &self.0 {
            if let Some(res) = hook.run(input) {
                return Some(res);
            }
        }
        None
    }

    pub fn tap(&mut self, hook: impl SyncBail<H> + 'static) {
        self.0.push(Box::new(hook) as Box<dyn SyncBail<H>>);
    }
}

pub struct SyncBailHookMap<H>(HashMap<String, SyncBailHook<H>>);

impl<H> Default for SyncBailHookMap<H> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<H: Hook> SyncBailHookMap<H> {
    pub fn tap(&mut self, key: String, hook: impl SyncBail<H> + 'static) {
        self.0
            .entry(key)
            .or_insert_with(|| SyncBailHook::default())
            .tap(hook);
    }

    pub fn call(&mut self, key: &str, input: &mut H::Input) -> Option<H::Output> {
        self.0.get_mut(key).and_then(|hooks| hooks.call(input))
    }
}

#[async_trait]
pub trait AsyncSeries<H: Hook> {
    async fn run(&self, input: &mut H::Input);
    fn stage(&self) -> i32 {
        0
    }
}

#[async_trait]
impl<H: Hook, F: Future<Output = ()> + Send, T: Fn(&mut H::Input) -> F + Sync> AsyncSeries<H>
    for T
{
    async fn run(&self, input: &mut H::Input) {
        self(input).await;
    }
}

#[async_trait]
impl<H: Hook, F: Future<Output = ()> + Send, T: Fn(&mut H::Input) -> F + Send + Sync> AsyncSeries<H>
    for (T, i32)
{
    async fn run(&self, input: &mut H::Input) {
        self.0(input).await;
    }
    fn stage(&self) -> i32 {
        self.1
    }
}

pub struct AsyncSeriesHook<H>(Vec<Box<dyn AsyncSeries<H>>>);

impl<H> Default for AsyncSeriesHook<H> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<H: Hook> AsyncSeriesHook<H> {
    pub async fn call(&mut self, input: &mut H::Input) {
        self.0.sort_by_key(|hook| hook.stage());
        for hook in &self.0 {
            hook.run(input).await;
        }
    }

    pub fn tap(&mut self, hook: impl AsyncSeries<H> + 'static) {
        self.0.push(Box::new(hook) as Box<dyn AsyncSeries<H>>);
    }
}

#[async_trait]
pub trait AsyncParallel<H: Hook> {
    async fn run(&self, input: &H::Input);
    fn stage(&self) -> i32 {
        0
    }
}

#[async_trait]
impl<H: Hook, F: Future<Output = ()> + Send, T: Fn(&H::Input) -> F + Sync> AsyncParallel<H> for T {
    async fn run(&self, input: &H::Input) {
        self(input).await;
    }
}

#[async_trait]
impl<H: Hook, F: Future<Output = ()> + Send, T: Fn(&H::Input) -> F + Send + Sync> AsyncParallel<H>
    for (T, i32)
{
    async fn run(&self, input: &H::Input) {
        self.0(input).await;
    }
    fn stage(&self) -> i32 {
        self.1
    }
}

pub struct AsyncParallelHook<H>(Vec<Box<dyn AsyncParallel<H>>>);

impl<H> Default for AsyncParallelHook<H> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<H: Hook> AsyncParallelHook<H> {
    pub async fn call(&mut self, input: &H::Input) {
        self.0.sort_by_key(|hook| hook.stage());
        let groupes = self.0.group_by(|a, b| a.stage() == b.stage());
        for group in groupes {
            let futs: Vec<_> = group.iter().map(|hook| hook.run(input)).collect();
            futs.join().await;
        }
    }

    pub fn tap(&mut self, hook: impl AsyncParallel<H> + 'static) {
        self.0.push(Box::new(hook) as Box<dyn AsyncParallel<H>>);
    }
}
