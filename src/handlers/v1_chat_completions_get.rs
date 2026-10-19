use uhttp::{Request, Response};

use crate::handler_context::HandlerContext;

pub async fn v1_chat_completions_get(_req: Request, res: Response, _ctx: HandlerContext) -> uhttp::HandlerResult {
    res.status(200).empty()
}