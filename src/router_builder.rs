use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use futures::Future;
use specta::TypeDefs;

use crate::{
    Config, ExecError, FirstMiddleware, LayerResult, MiddlewareContext, NextMiddleware, Procedure,
    Resolver, Router, StreamOrValue, StreamResolver,
};

pub struct RouterBuilder<
    TCtx = (), // The is the context the current router was initialised with
    TMeta = (),
    TLayerCtx = TCtx, // This is the context of the current layer -> Whatever the last middleware returned
> where
    TCtx: 'static,
    TLayerCtx: 'static,
{
    config: Config,
    middleware: Box<dyn Fn(NextMiddleware<TLayerCtx>) -> FirstMiddleware<TCtx>>,
    queries: HashMap<String, Procedure<TCtx>>,
    mutations: HashMap<String, Procedure<TCtx>>,
    subscriptions: HashMap<String, Procedure<TCtx>>,
    phantom: PhantomData<TMeta>,
}

impl<TCtx, TMeta> Router<TCtx, TMeta>
where
    TCtx: Send + 'static,
    TMeta: Send + Sync + 'static,
{
    pub fn new() -> RouterBuilder<TCtx, TMeta, TCtx> {
        RouterBuilder {
            config: Config::new(),
            middleware: Box::new(|next| Box::new(move |ctx, args, kak| next(ctx, args, kak))),
            queries: HashMap::new(),
            mutations: HashMap::new(),
            subscriptions: HashMap::new(),
            phantom: PhantomData,
        }
    }
}

impl<TCtx, TMeta> RouterBuilder<TCtx, TMeta>
where
    TCtx: Send + 'static,
    TMeta: Send + Sync + 'static,
{
    pub fn new() -> RouterBuilder<TCtx, TMeta, TCtx> {
        RouterBuilder {
            config: Config::new(),
            middleware: Box::new(|next| Box::new(move |ctx, args, kak| next(ctx, args, kak))),
            queries: HashMap::new(),
            mutations: HashMap::new(),
            subscriptions: HashMap::new(),
            phantom: PhantomData,
        }
    }
}

impl<TCtx, TMeta, TLayerCtx> RouterBuilder<TCtx, TMeta, TLayerCtx> {
    /// Attach a configuration to the router. Calling this multiple times will overwrite the previous config.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn middleware<TNewLayerCtx, TFut>(
        self,
        func: fn(MiddlewareContext<TLayerCtx, TNewLayerCtx>) -> TFut,
    ) -> RouterBuilder<TCtx, TMeta, TNewLayerCtx>
    where
        TNewLayerCtx: Send + 'static,
        TFut: Future<Output = Result<StreamOrValue, ExecError>> + Send + 'static,
    {
        let Self {
            config,
            middleware,
            queries,
            mutations,
            subscriptions,
            ..
        } = self;

        RouterBuilder {
            config,
            middleware: Box::new(move |nextmw| {
                // TODO: An `Arc` is more avoid than should be need but it's probs better than leaking memory.
                // I can't work out lifetimes to avoid this but would be great to try again!
                let nextmw = Arc::new(nextmw);

                (middleware)(Box::new(move |ctx, arg, (kind, key)| {
                    Ok(LayerResult::FutureStreamOrValue(Box::pin(func(
                        MiddlewareContext::<TLayerCtx, TNewLayerCtx> {
                            key,
                            kind,
                            ctx,
                            arg,
                            nextmw: nextmw.clone(),
                        },
                    ))))
                }))
            }),
            queries,
            mutations,
            subscriptions,
            phantom: PhantomData,
        }
    }

    pub fn query<TResolver, TMarker>(mut self, key: &'static str, resolver: TResolver) -> Self
    where
        TResolver: Resolver<TLayerCtx, TMarker> + Send + Sync + 'static,
    {
        let key = key.to_string();
        if self.queries.contains_key(&key) {
            panic!(
                "rspc error: query operation already has resolver with name '{}'",
                key
            );
        }

        self.queries.insert(
            key,
            Procedure {
                exec: (self.middleware)(Box::new(move |nextmw, arg, _| {
                    resolver.exec(
                        nextmw,
                        serde_json::from_value(arg).map_err(ExecError::DeserializingArgErr)?,
                    )
                })),
                ty: TResolver::typedef(&mut TypeDefs::default()),
            },
        );
        self
    }

    pub fn mutation<TResolver, TMarker>(mut self, key: &'static str, resolver: TResolver) -> Self
    where
        TResolver: Resolver<TLayerCtx, TMarker> + Send + Sync + 'static,
    {
        let key = key.to_string();
        if self.mutations.contains_key(&key) {
            panic!(
                "rspc error: mutation operation already has resolver with name '{}'",
                key
            );
        }

        self.mutations.insert(
            key,
            Procedure {
                exec: (self.middleware)(Box::new(move |nextmw, arg, _| {
                    resolver.exec(
                        nextmw,
                        serde_json::from_value(arg).map_err(ExecError::DeserializingArgErr)?,
                    )
                })),
                ty: TResolver::typedef(&mut TypeDefs::default()),
            },
        );
        self
    }

    pub fn subscription<TResolver, TMarker>(
        mut self,
        key: &'static str,
        resolver: TResolver,
    ) -> Self
    where
        TResolver: StreamResolver<TLayerCtx, TMarker> + Send + Sync + 'static,
    {
        let key = key.to_string();
        if self.subscriptions.contains_key(&key) {
            panic!(
                "rspc error: subscription operation already has resolver with name '{}'",
                key
            );
        }

        self.subscriptions.insert(
            key,
            Procedure {
                exec: (self.middleware)(Box::new(move |nextmw, arg, _| {
                    resolver.exec(
                        nextmw,
                        serde_json::from_value(arg).map_err(ExecError::DeserializingArgErr)?,
                    )
                })),
                ty: TResolver::typedef(&mut TypeDefs::default()),
            },
        );
        self
    }

    pub fn merge<TNewLayerCtx>(
        self,
        prefix: &'static str,
        router: RouterBuilder<TLayerCtx, TMeta, TNewLayerCtx>,
    ) -> RouterBuilder<TCtx, TMeta, TNewLayerCtx> {
        let Self {
            config,
            middleware,
            mut queries,
            mut mutations,
            mut subscriptions,
            ..
        } = self;

        for (key, query) in router.queries {
            queries.insert(
                format!("{}{}", prefix, key),
                Procedure {
                    exec: (middleware)(Box::new(query.exec)),
                    ty: query.ty,
                },
            );
        }

        for (key, mutation) in router.mutations {
            mutations.insert(
                format!("{}{}", prefix, key),
                Procedure {
                    exec: (middleware)(Box::new(mutation.exec)),
                    ty: mutation.ty,
                },
            );
        }

        for (key, subscription) in router.subscriptions {
            subscriptions.insert(
                format!("{}{}", prefix, key),
                Procedure {
                    exec: (middleware)(Box::new(subscription.exec)),
                    ty: subscription.ty,
                },
            );
        }

        RouterBuilder {
            config,
            middleware: Box::new(move |next| middleware((router.middleware)(next))),
            queries,
            mutations,
            subscriptions,
            phantom: PhantomData,
        }
    }

    pub fn build(self) -> Router<TCtx, TMeta> {
        let Self {
            queries,
            mutations,
            subscriptions,
            ..
        } = self;

        let router = Router {
            queries,
            mutations,
            subscriptions,
            phantom: PhantomData,
        };

        #[cfg(debug_assertions)]
        if let Some(export_path) = self.config.export_bindings_on_build {
            router.export_ts(export_path).unwrap();
        }

        router
    }
}
