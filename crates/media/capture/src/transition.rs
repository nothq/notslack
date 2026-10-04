use crate::MediaCaptureError;

pub(crate) struct CaptureTransition<State, Output> {
    pub(crate) next_state: Option<State>,
    pub(crate) result: Result<Output, MediaCaptureError>,
}

impl<State, Output> CaptureTransition<State, Output> {
    pub(crate) fn transitioned(next_state: State, output: Output) -> Self {
        Self {
            next_state: Some(next_state),
            result: Ok(output),
        }
    }

    pub(crate) fn completed(result: Result<Output, MediaCaptureError>) -> Self {
        Self {
            next_state: None,
            result,
        }
    }

    pub(crate) fn retained(state: State, error: MediaCaptureError) -> Self {
        Self {
            next_state: Some(state),
            result: Err(error),
        }
    }
}
