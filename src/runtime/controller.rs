use super::*;

use crate::config::ControllerBindings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ControlTarget {
    #[default]
    Head,
    Left,
    Right,
}

impl ControlTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Left => "Left controller",
            Self::Right => "Right controller",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ControllerSim {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub trigger: f32,
    pub grip: f32,
    pub primary: bool,
    pub secondary: bool,
    pub menu: bool,
    pub system: bool,
    pub stick_click: bool,
    pub thumbstick: xr::Vector2f,
}

impl ControllerSim {
    pub fn new(hand: u8) -> Self {
        Self {
            position: default_position(hand),
            yaw: 0.0,
            pitch: 0.0,
            trigger: 0.0,
            grip: 0.0,
            primary: false,
            secondary: false,
            menu: false,
            system: false,
            stick_click: false,
            thumbstick: xr::Vector2f { x: 0.0, y: 0.0 },
        }
    }

    pub fn reset(&mut self, hand: u8) {
        *self = Self::new(hand);
    }

    pub fn local_pose(self) -> xr::Posef {
        math::head_pose(self.yaw, self.pitch, self.position)
    }

    pub fn world_pose(self, head_world: xr::Posef) -> xr::Posef {
        math::pose_compose(head_world, self.local_pose())
    }

    pub fn sample_bindings(&mut self, bindings: &ControllerBindings) {
        let down = |key: &str| config::virtual_key(key).is_some_and(window::key_down);
        self.trigger = if down(&bindings.trigger) { 1.0 } else { 0.0 };
        self.grip = if down(&bindings.grip) { 1.0 } else { 0.0 };
        self.primary = down(&bindings.primary);
        self.secondary = down(&bindings.secondary);
        self.menu = down(&bindings.menu);
        self.system = down(&bindings.system);
        self.stick_click = down(&bindings.stick_click);

        let mut x = 0.0;
        let mut y = 0.0;
        if down(&bindings.stick_left) {
            x -= 1.0;
        }
        if down(&bindings.stick_right) {
            x += 1.0;
        }
        if down(&bindings.stick_down) {
            y -= 1.0;
        }
        if down(&bindings.stick_up) {
            y += 1.0;
        }
        self.thumbstick = xr::Vector2f { x, y };
    }

    pub fn boolean_state(&self, name: &str) -> bool {
        if action_name_matches(name, "trigger")
            || action_name_matches(name, "select")
            || action_name_matches(name, "fire")
        {
            self.trigger >= 0.5
        } else if action_name_matches(name, "squeeze") || action_name_matches(name, "grip") {
            self.grip >= 0.5
        } else if action_name_matches(name, "thumbstick_click") {
            self.stick_click
        } else if action_name_matches(name, "menu") {
            self.menu
        } else if action_name_matches(name, "system") {
            self.system
        } else if action_name_matches(name, "button_x") || action_name_matches(name, "button_a") {
            self.primary
        } else if action_name_matches(name, "button_y") || action_name_matches(name, "button_b") {
            self.secondary
        } else {
            false
        }
    }

    pub fn float_state(&self, name: &str) -> Option<f32> {
        if action_name_matches(name, "trigger")
            || action_name_matches(name, "select")
            || action_name_matches(name, "fire")
        {
            Some(self.trigger)
        } else if action_name_matches(name, "squeeze") || action_name_matches(name, "grip") {
            Some(self.grip)
        } else {
            None
        }
    }
}

fn default_position(hand: u8) -> Vec3 {
    let lateral = if hand == 0 { -0.18 } else { 0.18 };
    Vec3::new(lateral, -0.25, -0.15)
}

pub(super) fn controller_hand_for_modifiers(left: bool, right: bool) -> Option<usize> {
    if left {
        Some(0)
    } else if right {
        Some(1)
    } else {
        None
    }
}

pub(super) fn control_target_for_modifiers(left: bool, right: bool) -> ControlTarget {
    match controller_hand_for_modifiers(left, right) {
        Some(0) => ControlTarget::Left,
        Some(1) => ControlTarget::Right,
        _ => ControlTarget::Head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_world_pose_follows_the_head_frame() {
        let head = math::head_pose(0.5, 0.0, Vec3::new(0.2, 1.7, -0.4));
        let controller = ControllerSim::new(1);
        let world = controller.world_pose(head);
        let relative = math::pose_relative(world, head);
        assert!((relative.position.x - controller.position.x).abs() < 1e-5);
        assert!((relative.position.y - controller.position.y).abs() < 1e-5);
        assert!((relative.position.z - controller.position.z).abs() < 1e-5);
    }

    #[test]
    fn analog_and_boolean_button_states_match_action_names() {
        let mut controller = ControllerSim::new(1);
        controller.trigger = 0.8;
        controller.primary = true;
        assert!(controller.boolean_state("trigger"));
        assert_eq!(controller.float_state("trigger"), Some(0.8));
        assert!(controller.boolean_state("button_a"));
        assert!(!controller.boolean_state("button_b"));
    }

    #[test]
    fn left_modifier_wins_when_both_are_held() {
        assert_eq!(controller_hand_for_modifiers(true, true), Some(0));
        assert_eq!(controller_hand_for_modifiers(false, true), Some(1));
        assert_eq!(controller_hand_for_modifiers(false, false), None);
        assert_eq!(
            control_target_for_modifiers(true, false),
            ControlTarget::Left
        );
    }
}
