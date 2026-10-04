use std::time::Duration;

use crate::spawn_timer_task_for_entity;
use gpui::Context;

use super::{TextInput, CARET_BLINK_MS};

impl TextInput {
    pub(super) fn reset_caret_blink(&mut self) {
        self.caret_visible = true;
        self.blink_scheduled = false;
        self.blink_generation = self.blink_generation.wrapping_add(1);
    }

    pub(super) fn schedule_blink(&mut self, cx: &mut Context<Self>) {
        if self.blink_scheduled {
            return;
        }
        self.blink_scheduled = true;
        let generation = self.blink_generation;
        spawn_timer_task_for_entity(
            generation,
            Duration::from_millis(CARET_BLINK_MS),
            cx,
            |this, generation, cx| {
                if this.blink_generation != generation || !this.focused_last_render {
                    return;
                }
                this.caret_visible = !this.caret_visible;
                this.blink_scheduled = false;
                cx.notify();
            },
        );
    }
}
