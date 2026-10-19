use uhttp::Request;
use uhttp::Response;

use crate::handler_context::HandlerContext;

pub async fn health_any(
  _req: Request,
  res: Response,
  _ctx: HandlerContext,
) -> uhttp::HandlerResult {
  res.status(200).empty()
}
