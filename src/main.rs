use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_ggrs::{ggrs, AddRollbackCommandExtension, GgrsApp, GgrsPlugin, GgrsSchedule, LocalPlayers, PlayerInputs, ReadInputs};
use bevy_matchbox::prelude::*;
use components::Player;
use input::{direction, read_local_inputs};
mod input;
mod components;
pub type Config = bevy_ggrs::GgrsConfig<u8, PeerId>;
const MAP_SIZE: u32 = 41;
const GRID_WIDTH: f32 = 0.05;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // fill the entire browser window
                    fit_canvas_to_parent: true,
                    // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            }),
            GgrsPlugin::<Config>::default(),
        ))
        .rollback_component_with_clone::<Transform>()
        .insert_resource(ClearColor(Color::srgb(0.53, 0.53, 0.53)))
        .add_systems(Startup, (setup, spawn_players, start_matchbox_socket))
        .add_systems(Update, (wait_for_players, camera_follow))
        .add_systems(ReadInputs, read_local_inputs)
        .add_systems(GgrsSchedule, move_players)
        .run();
}

/// Commands 结构体是一个命令队列，用于对 World 进行结构变更。每个命令都需要独占访问 World，因此所有排队的命令会在 apply_deferred 系统运行时自动按顺序应用。
fn setup(mut commands: Commands) {
    // 实例化一个2d相机
    let mut camera_bundle = Camera2dBundle::default();
    // 设置了相机的投影模式为固定垂直缩放，并且指定了垂直缩放比例为 10.0。这意味着无论窗口的大小如何变化，相机的垂直视野将保持不变，而水平视野将根据窗口的宽高比进行调整。
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    // 向word中添加一个相机
    commands.spawn(camera_bundle);
       // Horizontal lines
       for i in 0..=MAP_SIZE {
        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                0.,
                i as f32 - MAP_SIZE as f32 / 2.,
                0.,
            )),
            sprite: Sprite {
                color: Color::srgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(MAP_SIZE as f32, GRID_WIDTH)),
                ..default()
            },
            ..default()
        });
    }

    // Vertical lines
    for i in 0..=MAP_SIZE {
        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                i as f32 - MAP_SIZE as f32 / 2.,
                0.,
                0.,
            )),
            sprite: Sprite {
                color: Color::srgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(GRID_WIDTH, MAP_SIZE as f32)),
                ..default()
            },
            ..default()
        });
    }
}
// 建立连接到 Matchbox 服务器
fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "ws://127.0.0.1:3536/extreme_bevy?next=2";
    info!("connecting to matchbox server: {room_url}");
    commands.insert_resource(MatchboxSocket::new_ggrs(room_url));
}
/// 初始化玩家
fn spawn_players(mut commands: Commands) {
    // Player 1
    commands
        .spawn((
            Player { handle: 0 },
            SpriteBundle {
                transform: Transform::from_translation(Vec3::new(-2., 0., 100.)), // new
                sprite: Sprite {
                    color: Color::srgb(0., 0.47, 1.),
                    custom_size: Some(Vec2::new(1., 1.)),
                    ..default()
                },
                ..default()
            },
        ))
        .add_rollback();

    // Player 2
    commands
        .spawn((
            Player { handle: 1 },
            SpriteBundle {
                transform: Transform::from_translation(Vec3::new(2., 0., 100.)), // new
                sprite: Sprite {
                    color: Color::srgb(0., 0.4, 0.),
                    custom_size: Some(Vec2::new(1., 1.)),
                    ..default()
                },
                ..default()
            },
        ))
        .add_rollback();
}
/// 等待玩家加入
 fn wait_for_players(mut commands: Commands, mut socket: ResMut<MatchboxSocket<SingleChannel>>) {
    // 检查是否建立通道
   if socket.get_channel(0).is_err() {
       return; // we've already started
   }

   // Check for new connections
   socket.update_peers();
   let players = socket.players();
   // 检查玩家数量
   let num_players = 2;
   if players.len() < num_players {
       return; // wait for more players
   }

   info!("All peers have joined, going in-game");

   // create a GGRS P2P session
   let mut session_builder = ggrs::SessionBuilder::<Config>::new()
       .with_num_players(num_players)//设置玩家数量
       .with_input_delay(2);//设置输入延迟，单位帧

   for (i, player) in players.into_iter().enumerate() {
       session_builder = session_builder
           .add_player(player, i)
           .expect("failed to add player");
   }

   // move the channel out of the socket (required because GGRS takes ownership of it)
   let channel = socket.take_channel(0).unwrap();

   // start the GGRS session
   let ggrs_session = session_builder
       .start_p2p_session(channel)
       .expect("failed to start session");

   commands.insert_resource(bevy_ggrs::Session::P2P(ggrs_session));
}
/// 玩家移动
fn move_players(
    mut players: Query<(&mut Transform, &Player)>,
    inputs: Res<PlayerInputs<Config>>,
    time: Res<Time>,
) {
    for (mut transform, player) in &mut players {
        let (input, _) = inputs[player.handle];

        let direction = direction(input);

        if direction == Vec2::ZERO {
            continue;
        }

        let move_speed = 7.;
        let move_delta = direction * move_speed * time.delta_seconds();

        let old_pos = transform.translation.xy();
        let limit = Vec2::splat(MAP_SIZE as f32 / 2. - 0.5);
        let new_pos = (old_pos + move_delta).clamp(-limit, limit);

        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
    }
}
/// 摄像机跟随
fn camera_follow(
    local_players: Res<LocalPlayers>,
    players: Query<(&Player, &Transform)>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    for (player, player_transform) in &players {
        // only follow the local player
        if !local_players.0.contains(&player.handle) {
            continue;
        }

        let pos = player_transform.translation;

        for mut transform in &mut cameras {
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
    }
}