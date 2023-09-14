#![feature(slice_group_by)]

use std::sync::Arc;

use async_trait::async_trait;
use futures_concurrency::prelude::*;

struct Compilation;

#[derive(Default)]
struct CompilationHooks {
    make: Vec<Arc<dyn AsyncParallel<Make>>>,
    process_assets: Vec<Arc<dyn AsyncSeries<ProcessAssets>>>,
}

impl CompilationHooks {
    async fn call_make(&mut self, input: &Compilation) {
        self.make.sort_by_key(|hook| hook.stage());
        let groupes = self.make.group_by(|a, b| a.stage() == b.stage());
        for group in groupes {
            execute_async_parallel(group, input).await;
        }
    }
    async fn call_process_assets(&mut self, input: &mut Compilation) {
        self.process_assets.sort_by_key(|hook| hook.stage());
        execute_async_series(&self.process_assets, input).await;
    }
}

#[async_trait::async_trait]
trait AsyncSeries<H: Hook> {
    async fn run(&self, input: &mut H::Input) -> H::Output;
    fn stage(&self) -> i32 {
        0
    }
}

async fn execute_async_series<'c, I, H: Hook<Input = I, Output = ()>>(
    hooks: &[Arc<dyn AsyncSeries<H>>],
    input: &'c mut I,
) {
    for hook in hooks {
        hook.run(input).await;
    }
}

#[async_trait::async_trait]
trait AsyncParallel<H: Hook> {
    async fn run(&self, input: &H::Input) -> H::Output;
    fn stage(&self) -> i32 {
        0
    }
}

async fn execute_async_parallel<'c, I, H: Hook<Input = I, Output = ()>>(
    hooks: &[Arc<dyn AsyncParallel<H>>],
    input: &'c I,
) {
    let futs: Vec<_> = hooks.into_iter().map(|hook| hook.run(input)).collect();
    futs.join().await;
}

trait Hook {
    type Input;
    type Output;
}

struct Make;

impl Hook for Make {
    type Input = Compilation;
    type Output = ();
}

struct ProcessAssets;

impl Hook for ProcessAssets {
    type Input = Compilation;
    type Output = ();
}

trait Plugin<HookContainer> {
    fn tap_hooks(&self, hook_container: &mut HookContainer);
}

struct APlugin;

#[async_trait::async_trait]
impl AsyncParallel<Make> for APlugin {
    async fn run(&self, _compilation: &Compilation) {
        println!("APlugin: make {}", AsyncParallel::stage(self));
    }
}

#[async_trait]
impl AsyncSeries<ProcessAssets> for APlugin {
    async fn run(&self, _compilation: &mut Compilation) {
        println!("APlugin: processAssets {}", AsyncSeries::stage(self));
    }
}

impl Plugin<CompilationHooks> for Arc<APlugin> {
    fn tap_hooks(&self, hook_container: &mut CompilationHooks) {
        hook_container.make.push(self.clone());
        hook_container.process_assets.push(self.clone());
    }
}

struct BPlugin;

#[async_trait::async_trait]
impl AsyncParallel<Make> for BPlugin {
    async fn run(&self, _compilation: &Compilation) {
        println!("BPlugin: make {}", AsyncParallel::stage(self));
    }
}

#[async_trait]
impl AsyncSeries<ProcessAssets> for BPlugin {
    async fn run(&self, _compilation: &mut Compilation) {
        println!("BPlugin: processAssets {}", AsyncSeries::stage(self));
    }
    fn stage(&self) -> i32 {
        -100
    }
}

impl Plugin<CompilationHooks> for Arc<BPlugin> {
    fn tap_hooks(&self, hook_container: &mut CompilationHooks) {
        hook_container.make.push(self.clone());
        hook_container.process_assets.push(self.clone());
    }
}

#[tokio::main]
async fn main() {
    let mut compilation_hooks = CompilationHooks::default();
    let a_plugin = Arc::new(APlugin);
    a_plugin.tap_hooks(&mut compilation_hooks);
    let b_plugin = Arc::new(BPlugin);
    b_plugin.tap_hooks(&mut compilation_hooks);

    let mut compilation = Compilation;
    compilation_hooks.call_make(&compilation).await;
    compilation_hooks
        .call_process_assets(&mut compilation)
        .await;
}
