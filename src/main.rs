#![feature(slice_group_by)]
#![feature(never_type)]

mod hook;

use hook::{AsyncParallelHook, AsyncSeriesHook, Hook, Plugin, SyncBailHookMap};

struct Compilation;

struct Make;

impl Hook for Make {
    type Input = Compilation;
    type Output = !;
}

struct ProcessAssets;

impl Hook for ProcessAssets {
    type Input = Compilation;
    type Output = !;
}

#[derive(Default)]
struct CompilationHooks {
    make: AsyncParallelHook<Make>,
    process_assets: AsyncSeriesHook<ProcessAssets>,
}

struct Ast;

#[derive(Debug)]
struct BasicEvaluatedExpression;

struct Evaluate;

impl Hook for Evaluate {
    type Input = Ast;
    type Output = BasicEvaluatedExpression;
}

#[derive(Default)]
struct ParserHooks {
    evaluate: SyncBailHookMap<Evaluate>,
}

struct APlugin;

impl Plugin<CompilationHooks> for APlugin {
    fn apply(&self, hook_container: &mut CompilationHooks) {
        hook_container.make.tap((
            |_compilation: &Compilation| async {
                println!("APlugin: CompilationHooks.make -10");
            },
            -10,
        ));
        hook_container
            .process_assets
            .tap(|_compilation: &mut Compilation| async {
                println!("APlugin: CompilationHooks.processAssets");
            });
    }
}

impl Plugin<ParserHooks> for APlugin {
    fn apply(&self, hook_container: &mut ParserHooks) {
        hook_container
            .evaluate
            .tap("CallExpression".to_string(), |_ast: &mut Ast| {
                println!("APlugin: ParserHooks.evaluate");
                Some(BasicEvaluatedExpression)
            });
    }
}

struct BPlugin;

impl Plugin<CompilationHooks> for BPlugin {
    fn apply(&self, hook_container: &mut CompilationHooks) {
        hook_container.make.tap(|_compilation: &Compilation| async {
            println!("BPlugin: CompilationHooks.make");
        });
        hook_container.process_assets.tap((
            |_compilation: &mut Compilation| async {
                println!("BPlugin: CompilationHooks.processAssets -100");
            },
            -100,
        ));
    }
}

#[tokio::main]
async fn main() {
    let mut compilation_hooks = CompilationHooks::default();
    let mut parser_hooks = ParserHooks::default();
    let a_plugin = APlugin;
    a_plugin.apply(&mut compilation_hooks);
    a_plugin.apply(&mut parser_hooks);
    let b_plugin = BPlugin;
    b_plugin.apply(&mut compilation_hooks);

    let mut compilation = Compilation;
    let mut ast = Ast;

    let _evaluated = parser_hooks.evaluate.call("CallExpression", &mut ast);
    compilation_hooks.make.call(&compilation).await;
    compilation_hooks
        .process_assets
        .call(&mut compilation)
        .await;
}
