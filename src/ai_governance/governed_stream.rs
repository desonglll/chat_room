use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use uuid::Uuid;

use crate::{
    ai::{AiStreamItem, AiTextStream},
    state::SharedState,
};

pub(crate) struct GovernedAiStream {
    inner: AiTextStream,
    state: SharedState,
    admission_id: Option<Uuid>,
    input_tokens: i64,
    output_chars: i64,
}

impl GovernedAiStream {
    pub(crate) fn new(
        inner: AiTextStream,
        state: SharedState,
        admission_id: Uuid,
        input_tokens: i64,
    ) -> Self {
        Self {
            inner,
            state,
            admission_id: Some(admission_id),
            input_tokens,
            output_chars: 0,
        }
    }

    fn finish(&mut self, status: &'static str) {
        let Some(admission_id) = self.admission_id.take() else {
            return;
        };
        let state = self.state.clone();
        let input_tokens = self.input_tokens;
        let output_tokens = self.output_chars.saturating_add(3) / 4;
        tokio::spawn(async move {
            if let Err(error) = state
                .finish_ai_admission(admission_id, status, Some(input_tokens), output_tokens)
                .await
            {
                tracing::error!("record streamed AI usage failed: {error}");
            }
        });
    }
}

impl Stream for GovernedAiStream {
    type Item = anyhow::Result<AiStreamItem>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(AiStreamItem::Content(chunk)))) => {
                self.output_chars = self
                    .output_chars
                    .saturating_add(chunk.chars().count() as i64);
                Poll::Ready(Some(Ok(AiStreamItem::Content(chunk))))
            }
            Poll::Ready(Some(Ok(AiStreamItem::Reasoning))) => {
                Poll::Ready(Some(Ok(AiStreamItem::Reasoning)))
            }
            Poll::Ready(Some(Err(error))) => {
                self.finish("failed");
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                self.finish("completed");
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for GovernedAiStream {
    fn drop(&mut self) {
        self.finish("failed");
    }
}
