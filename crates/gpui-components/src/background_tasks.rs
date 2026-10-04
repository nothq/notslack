//! GPUI entity task helpers.

use std::future::Future;
use std::time::Duration;

use gpui::Context;

pub trait BackgroundTaskRequest: Send + 'static {}

impl<T> BackgroundTaskRequest for T where T: Send + 'static {}

pub trait BackgroundTaskResult: Send + 'static {}

impl<T> BackgroundTaskResult for T where T: Send + 'static {}

pub fn spawn_background_task_for_entity<Entity, Request, Result, Work, Apply>(
    request: Request,
    cx: &mut Context<Entity>,
    work: Work,
    apply: Apply,
) where
    Entity: 'static,
    Request: BackgroundTaskRequest,
    Result: BackgroundTaskResult,
    Work: FnOnce(Request) -> Result + Send + 'static,
    Apply: FnOnce(&mut Entity, Result, &mut Context<Entity>) + 'static,
{
    let task = cx.background_executor().spawn(async move { work(request) });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| apply(this, result, cx));
    })
    .detach();
}

pub fn spawn_async_background_task_for_entity<Entity, Request, Result, Work, Task, Apply>(
    request: Request,
    cx: &mut Context<Entity>,
    work: Work,
    apply: Apply,
) where
    Entity: 'static,
    Request: BackgroundTaskRequest,
    Result: BackgroundTaskResult,
    Work: FnOnce(Request) -> Task + Send + 'static,
    Task: Future<Output = Result> + Send + 'static,
    Apply: FnOnce(&mut Entity, Result, &mut Context<Entity>) + 'static,
{
    let task = cx
        .background_executor()
        .spawn(async move { work(request).await });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| apply(this, result, cx));
    })
    .detach();
}

pub fn spawn_timer_task_for_entity<Entity, Token, Apply>(
    token: Token,
    delay: Duration,
    cx: &mut Context<Entity>,
    apply: Apply,
) where
    Entity: 'static,
    Token: Send + 'static,
    Apply: FnOnce(&mut Entity, Token, &mut Context<Entity>) + 'static,
{
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(delay).await;
        let _ = this.update(cx, |this, cx| apply(this, token, cx));
    })
    .detach();
}
