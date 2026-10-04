use axum::http::HeaderValue;

pub(super) struct SlackMediaRange {
    header: String,
    start: Option<u64>,
    end: Option<u64>,
}

impl SlackMediaRange {
    pub(super) fn parse(header: Option<&HeaderValue>) -> Result<Option<Self>, ()> {
        let Some(header) = header else {
            return Ok(None);
        };
        let value = header.to_str().map_err(|_| ())?;
        let spec = value.strip_prefix("bytes=").ok_or(())?;
        if spec.contains(',') {
            return Err(());
        }
        let (start, end) = spec.split_once('-').ok_or(())?;
        if start.is_empty() && end.is_empty() {
            return Err(());
        }
        let start = (!start.is_empty())
            .then(|| start.parse::<u64>())
            .transpose()
            .map_err(|_| ())?;
        let end = (!end.is_empty())
            .then(|| end.parse::<u64>())
            .transpose()
            .map_err(|_| ())?;
        if start.zip(end).is_some_and(|(start, end)| start > end)
            || start.is_none() && end == Some(0)
        {
            return Err(());
        }
        Ok(Some(Self {
            header: value.to_string(),
            start,
            end,
        }))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.header
    }

    pub(super) fn validate_satisfied(
        &self,
        response_start: u64,
        response_end: u64,
        total: u64,
    ) -> Result<(), String> {
        let (expected_start, expected_end) = match (self.start, self.end) {
            (Some(start), Some(end)) => (start, end.min(total - 1)),
            (Some(start), None) => (start, total - 1),
            (None, Some(suffix_length)) => (total.saturating_sub(suffix_length), total - 1),
            (None, None) => unreachable!("validated range must include one endpoint"),
        };
        if response_start != expected_start || response_end != expected_end {
            return Err(format!(
                "Slack media returned bytes {response_start}-{response_end}/{total} for requested {}",
                self.header
            ));
        }
        Ok(())
    }

    pub(super) fn validate_unsatisfied(&self, total: u64) -> Result<(), String> {
        if self.start.is_some_and(|start| start >= total) {
            return Ok(());
        }
        Err(format!(
            "Slack media rejected satisfiable requested range {} for a {total} byte source",
            self.header
        ))
    }
}
