use bevy::{
    audio::Volume,
    math::bounding::{Aabb2d, BoundingCircle, IntersectsVolume},
    prelude::*,
    window::WindowResolution,
};
mod stepping;

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SIZE: Vec2 = Vec2::new(120.0, 20.0);
const GAP_BETWEEN_PADDLE_AND_FLOOR: f32 = 20.0;
const PADDLE_SPEED: f32 = 500.0;
// How close can the paddle get to the wall
const PADDLE_PADDING: f32 = 10.0;

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -25.0, 1.0);
const BALL_DIAMETER: f32 = 20.;
const BALL_SPEED: f32 = 400.0;
const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);

const WALL_THICKNESS: f32 = 10.0;
// x coordinates
const LEFT_WALL: f32 = -450.;
const RIGHT_WALL: f32 = 450.;
// y coordinates
const BOTTOM_WALL: f32 = -300.;
const TOP_WALL: f32 = 300.;

const BRICK_SIZE: Vec2 = Vec2::new(100., 30.);
// These values are exact
const GAP_BETWEEN_PADDLE_AND_BRICKS: f32 = 270.0;
const GAP_BETWEEN_BRICKS: f32 = 5.0;
// These values are lower bounds, as the number of bricks is computed
const GAP_BETWEEN_BRICKS_AND_CEILING: f32 = 20.0;
const GAP_BETWEEN_BRICKS_AND_SIDES: f32 = 20.0;

const SCOREBOARD_FONT_SIZE: f32 = 33.0;
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

const BACKGROUND_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const PADDLE_COLOR: Color = Color::srgb(0.3, 0.3, 0.7);
const BALL_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);
const BRICK_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
const WALL_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const TEXT_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(1600., 1000.),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(
            stepping::SteppingPlugin::default()
                .add_schedule(Update)
                .add_schedule(FixedUpdate)
                .at(Val::Percent(35.0), Val::Percent(50.0)),
        )
        .init_resource::<Game>()
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .init_state::<GameState>()
        .add_event::<CollisionEvent>()
        .add_systems(Startup, setup)
        // .add_systems(OnEnter(GameState::Playing), )
        .add_systems(
            FixedUpdate,
            (
                await_play.run_if(in_state(GameState::Paused)),
                apply_velocity.run_if(in_state(GameState::Playing)),
                move_paddle,
                check_for_collision,
                next_level,
                play_collision_sound,
            )
                .chain(),
        )
        .add_systems(Update, update_scoreboard)
        .add_systems(
            Update,
            game_over_keyboard.run_if(in_state(GameState::GameOver)),
        )
        .add_systems(OnExit(GameState::GameOver), reset_game)
        .run();
}

#[derive(Component)]
struct Paddle;

#[derive(Component)]
struct Ball;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Event, Default)]
struct CollisionEvent;

#[derive(Component)]
struct Brick;

#[derive(Resource, Deref)]
struct CollisionSound(Handle<AudioSource>);

// Default must be implemented to define this as a required component for the Wall component below
#[derive(Component, Default)]
struct Collider;

// This is a collection of the components that define a "Wall" in our game
#[derive(Component)]
#[require(Sprite, Transform, Collider)]
struct Wall;

/// Which side of the arena is this wall located on?
enum WallLocation {
    Left,
    Right,
    Bottom,
    Top,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum GameState {
    #[default]
    Paused,
    Playing,
    GameOver,
}

#[derive(Resource, Default)]
struct Game {
    level: u32,
    // lives: u32,
    bricks: u32,
    score: u32,
}

impl WallLocation {
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL, 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL, 0.),
            WallLocation::Bottom => Vec2::new(0., BOTTOM_WALL),
            WallLocation::Top => Vec2::new(0., TOP_WALL),
        }
    }

    fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        assert!(arena_width > 0.0);
        assert!(arena_height > 0.0);

        match self {
            WallLocation::Left | WallLocation::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            WallLocation::Bottom | WallLocation::Top => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
        }
    }
}

impl Wall {
    fn new(location: WallLocation) -> (Wall, Sprite, Transform) {
        (
            Wall,
            Sprite::from_color(WALL_COLOR, Vec2::ONE),
            Transform {
                translation: location.position().extend(0.0),
                scale: location.size().extend(1.0),
                ..default()
            },
        )
    }
}

#[derive(Component)]
struct ScoreboardUi;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game: ResMut<Game>,
    mut next_state: ResMut<NextState<GameState>>,
    asset_server: Res<AssetServer>,
) {
    next_state.set(GameState::Paused);
    // camera
    commands.spawn(Camera2d);

    // sound
    let ball_collision_sound = asset_server.load("sounds/breakout_collision.ogg");
    commands.insert_resource(CollisionSound(ball_collision_sound));

    // paddle
    let paddle_y = BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR;

    commands.spawn((
        Sprite::from_color(PADDLE_COLOR, Vec2::ONE),
        Transform {
            translation: Vec3::new(0.0, paddle_y, 0.0),
            scale: PADDLE_SIZE.extend(1.0),
            ..default()
        },
        Paddle,
        Collider,
    ));

    // ball
    commands.spawn((
        Mesh2d(meshes.add(Circle::default())),
        MeshMaterial2d(materials.add(BALL_COLOR)),
        Transform::from_translation(BALL_STARTING_POSITION)
            .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
        Ball,
        Velocity(INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED),
    ));

    // Scoreboard
    commands.spawn((
        Text::new("Score: "),
        TextFont {
            font_size: SCOREBOARD_FONT_SIZE,
            ..default()
        },
        TextColor(TEXT_COLOR),
        ScoreboardUi,
        Node {
            position_type: PositionType::Absolute,
            top: SCOREBOARD_TEXT_PADDING,
            left: SCOREBOARD_TEXT_PADDING,
            ..default()
        },
        children![(
            TextSpan::default(),
            TextFont {
                font_size: SCOREBOARD_FONT_SIZE,
                ..default()
            },
            TextColor(SCORE_COLOR),
        )],
    ));

    // Walls
    commands.spawn(Wall::new(WallLocation::Left));
    commands.spawn(Wall::new(WallLocation::Right));
    commands.spawn(Wall::new(WallLocation::Top));
    commands.spawn(Wall::new(WallLocation::Bottom));

    // Bricks
    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // Given the space available, compute how many rows and columns of bricks we can fit.
    let n_columns = (total_width_of_bricks / (BRICK_SIZE.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (BRICK_SIZE.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // Because we need to round the number of columns, the space on the top of the bricks only
    // captures a lower bound, not an exact value
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        // space taken up by the bricks
        - (n_columns as f32 / 2.0 * BRICK_SIZE.x)
        // space taken up by the gaps
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // In bevy, the 'transition' of an entity describes the center point, not its bottom-left
    // corner
    let offset_x = left_edge_of_bricks + BRICK_SIZE.x / 2.;
    let offset_y = bottom_edge_of_bricks + BRICK_SIZE.y / 2.;

    for row in 0..n_rows {
        for column in 0..n_columns {
            let brick_position = Vec2::new(
                offset_x + column as f32 * (BRICK_SIZE.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (BRICK_SIZE.y + GAP_BETWEEN_BRICKS),
            );
            game.bricks += 1;
            commands.spawn((
                Sprite {
                    color: BRICK_COLOR,
                    ..default()
                },
                Transform {
                    translation: brick_position.extend(0.0),
                    scale: Vec3::new(BRICK_SIZE.x, BRICK_SIZE.y, 1.0),
                    ..default()
                },
                Brick,
                Collider,
            ));
        }
    }
}

fn reset_game() {
    info!("reset");
}

fn move_paddle(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut paddle_transform: Single<&mut Transform, With<Paddle>>,
    time: Res<Time>,
) {
    let mut direction = 0.0;

    if keyboard_input.pressed(KeyCode::KeyA) {
        direction -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        direction += 1.0;
    }

    // calculate the new horizontal paddle position based on player input
    let new_paddle_position =
        paddle_transform.translation.x + direction * PADDLE_SPEED * time.delta_secs();

    // Update the paddle position, making sure it doesn't cause the paddle to leave the arena
    let left_bound = LEFT_WALL + WALL_THICKNESS / 2.0 + PADDLE_SIZE.x / 2.0 - PADDLE_PADDING;
    let right_bound = RIGHT_WALL + WALL_THICKNESS / 2.0 - PADDLE_SIZE.x / 2.0 - PADDLE_PADDING;

    paddle_transform.translation.x = new_paddle_position.clamp(left_bound, right_bound);
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}

fn update_scoreboard(
    game: ResMut<Game>,
    score_root: Single<Entity, (With<ScoreboardUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(*score_root, 1) = game.score.to_string();
}

fn check_for_collision(
    mut commands: Commands,
    mut game: ResMut<Game>,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    brick_collision_query: Query<(Entity, &Transform, Option<&Brick>), With<Collider>>,
    wall_collision_query: Query<(Entity, &Transform, Option<&Wall>), With<Collider>>,
    mut collision_events: EventWriter<CollisionEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();

    for (_, collider_transform, maybe_wall) in &wall_collision_query {
        let collision = ball_collision(
            BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
            Aabb2d::new(
                collider_transform.translation.truncate(),
                collider_transform.scale.truncate() / 2.,
            ),
        );
        if let Some(collision) = collision {
            if maybe_wall.is_some() && collision == Collision::Top {
                next_state.set(GameState::GameOver);
                ball_velocity.x = 0.0;
                ball_velocity.y = 0.0;
                return;
            }
        }
    }

    for (collider_entity, collider_transform, maybe_brick) in &brick_collision_query {
        let collision = ball_collision(
            BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
            Aabb2d::new(
                collider_transform.translation.truncate(),
                collider_transform.scale.truncate() / 2.,
            ),
        );

        if let Some(collision) = collision {
            collision_events.write_default();

            if maybe_brick.is_some() {
                commands.entity(collider_entity).despawn();
                game.score += 1;
                game.bricks -= 1;
            }
            let mut reflect_x = false;
            let mut reflect_y = false;

            match collision {
                Collision::Left => reflect_x = ball_velocity.x > 0.0,
                Collision::Right => reflect_x = ball_velocity.x < 0.0,
                Collision::Top => reflect_y = ball_velocity.y < 0.0,
                Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
            }

            if reflect_x {
                ball_velocity.x = -ball_velocity.x;
            }
            if reflect_y {
                ball_velocity.y = -ball_velocity.y;
            }
        }
    }
}

fn play_collision_sound(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    sound: Res<CollisionSound>,
) {
    if !collision_events.is_empty() {
        collision_events.clear();
        commands.spawn((
            AudioPlayer(sound.clone()),
            PlaybackSettings {
                volume: Volume::Linear(0.5),
                muted: false,
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Collision {
    Left,
    Right,
    Top,
    Bottom,
}

fn ball_collision(ball: BoundingCircle, bounding_box: Aabb2d) -> Option<Collision> {
    if !ball.intersects(&bounding_box) {
        return None;
    }

    let closest = bounding_box.closest_point(ball.center);
    let offset = ball.center - closest;
    let side = if offset.x.abs() > offset.y.abs() {
        if offset.x < 0. {
            Collision::Left
        } else {
            Collision::Right
        }
    } else if offset.y > 0. {
        Collision::Top
    } else {
        Collision::Bottom
    };

    Some(side)
}

fn next_level(mut commands: Commands, mut game: ResMut<Game>) {
    if game.bricks == 0 {
        game.level += 1;

        let paddle_y = BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR;
        // Bricks
        let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
        let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
        let total_height_of_bricks =
            TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

        assert!(total_width_of_bricks > 0.0);
        assert!(total_height_of_bricks > 0.0);

        // Given the space available, compute how many rows and columns of bricks we can fit.
        let n_columns =
            (total_width_of_bricks / (BRICK_SIZE.x + GAP_BETWEEN_BRICKS)).floor() as usize;
        let n_rows =
            (total_height_of_bricks / (BRICK_SIZE.y + GAP_BETWEEN_BRICKS)).floor() as usize;
        let n_vertical_gaps = n_columns - 1;

        // Because we need to round the number of columns, the space on the top of the bricks only
        // captures a lower bound, not an exact value
        let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
        let left_edge_of_bricks = center_of_bricks
            // space taken up by the bricks
            - (n_columns as f32 / 2.0 * BRICK_SIZE.x)
            // space taken up by the gaps
            - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

        // In bevy, the 'transition' of an entity describes the center point, not its bottom-left
        // corner
        let offset_x = left_edge_of_bricks + BRICK_SIZE.x / 2.;
        let offset_y = bottom_edge_of_bricks + BRICK_SIZE.y / 2.;

        for row in 0..n_rows {
            for column in 0..n_columns {
                let brick_position = Vec2::new(
                    offset_x + column as f32 * (BRICK_SIZE.x + GAP_BETWEEN_BRICKS),
                    offset_y + row as f32 * (BRICK_SIZE.y + GAP_BETWEEN_BRICKS),
                );

                game.bricks += 1;
                commands.spawn((
                    Sprite {
                        color: BRICK_COLOR,
                        ..default()
                    },
                    Transform {
                        translation: brick_position.extend(0.0),
                        scale: Vec3::new(BRICK_SIZE.x, BRICK_SIZE.y, 1.0),
                        ..default()
                    },
                    Brick,
                    Collider,
                ));
            }
        }
    }
}

fn await_play(
    mut next_state: ResMut<NextState<GameState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    info!("waiting");
    if keyboard_input.just_pressed(KeyCode::Space) {
        info!("start");
        next_state.set(GameState::Playing);
    }
}
fn game_over_keyboard(
    mut next_state: ResMut<NextState<GameState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Paused);
    }
}
