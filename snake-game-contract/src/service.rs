// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{ComplexObject, EmptySubscription, Object, Request, Response, Schema};
use linera_sdk::{linera_base_types::WithServiceAbi, views::View, Service, ServiceRuntime};
use snake_game::{SnakeGameAbi, GameSession, LeaderboardEntry};

use self::state::{SnakeGameState, PlayerStats};

linera_sdk::service!(SnakeGameService);

pub struct SnakeGameService {
    state: SnakeGameState,
    runtime: Arc<ServiceRuntime<Self>>,
}

impl WithServiceAbi for SnakeGameService {
    type Abi = SnakeGameAbi;
}

impl Service for SnakeGameService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = SnakeGameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        SnakeGameService {
            state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        // Collect all sessions
        let mut all_sessions = Vec::new();
        if let Ok(session_ids) = self.state.sessions.indices().await {
            for session_id in session_ids {
                if let Ok(Some(session)) = self.state.sessions.get(&session_id).await {
                    all_sessions.push(session);
                }
            }
        }
        
        // Get leaderboard data
        let global_leaderboard = self.state.global_leaderboard.get().clone();
        
        // Get player stats
        let mut all_player_stats = Vec::new();
        if let Ok(player_chains) = self.state.player_stats.indices().await {
            for player_chain in player_chains {
                if let Ok(Some(stats)) = self.state.player_stats.get(&player_chain).await {
                    all_player_stats.push(stats);
                }
            }
        }
        
        // Get personal data
        let my_sessions = self.state.my_sessions.get().clone();
        let my_stats = self.state.my_stats.get().clone();
        let my_current_session = self.state.my_current_session.get().clone();
        let my_player_name = self.state.my_player_name.get().clone();
        
        // Get all player names
        let mut all_player_names = Vec::new();
        if let Ok(chain_ids) = self.state.player_names.indices().await {
            for chain_id in chain_ids {
                if let Ok(Some(name)) = self.state.player_names.get(&chain_id).await {
                    all_player_names.push(PlayerNameEntry {
                        chain_id: chain_id.to_string(),
                        name,
                    });
                }
            }
        }
        
        // Get configuration
        let is_leaderboard_chain = *self.state.is_leaderboard_chain.get();
        let leaderboard_chain_id = self.state.leaderboard_chain_id.get().clone();
        let session_counter = *self.state.session_counter.get();
        
        let schema = Schema::build(
            QueryRoot {
                all_sessions,
                global_leaderboard,
                all_player_stats,
                my_sessions,
                my_stats,
                my_current_session,
                is_leaderboard_chain,
                leaderboard_chain_id,
                session_counter,
                my_player_name,
                all_player_names,
            },
            MutationRoot {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish();
        
        schema.execute(request).await
    }
}

struct QueryRoot {
    all_sessions: Vec<GameSession>,
    global_leaderboard: Vec<LeaderboardEntry>,
    all_player_stats: Vec<PlayerStats>,
    my_sessions: Vec<String>,
    my_stats: Option<PlayerStats>,
    my_current_session: Option<String>,
    is_leaderboard_chain: bool,
    leaderboard_chain_id: Option<linera_sdk::linera_base_types::ChainId>,
    session_counter: u64,
    my_player_name: Option<String>,
    all_player_names: Vec<PlayerNameEntry>,
}

#[Object]
impl QueryRoot {
    /// Get all game sessions
    async fn all_sessions(&self) -> &Vec<GameSession> {
        &self.all_sessions
    }
    
    /// Get a specific game session by ID
    async fn session(&self, session_id: String) -> Option<&GameSession> {
        self.all_sessions.iter().find(|session| session.session_id == session_id)
    }
    
    /// Get the global leaderboard
    async fn global_leaderboard(&self) -> &Vec<LeaderboardEntry> {
        &self.global_leaderboard
    }
    
    /// Get all player statistics
    async fn all_player_stats(&self) -> &Vec<PlayerStats> {
        &self.all_player_stats
    }
    
    /// Get player statistics for a specific chain
    async fn player_stats(&self, chain_id: String) -> Option<&PlayerStats> {
        // Parse chain_id string to ChainId if needed
        self.all_player_stats.iter().find(|stats| {
            format!("{:?}", stats.chain_id).contains(&chain_id)
        })
    }
    
    /// Get sessions this player participated in
    async fn my_sessions(&self) -> &Vec<String> {
        &self.my_sessions
    }
    
    /// Get personal statistics
    async fn my_stats(&self) -> &Option<PlayerStats> {
        &self.my_stats
    }
    
    /// Get current active session
    async fn my_current_session(&self) -> &Option<String> {
        &self.my_current_session
    }
    
    /// Check if this chain is the leaderboard chain
    async fn is_leaderboard_chain(&self) -> bool {
        self.is_leaderboard_chain
    }
    
    /// Get the configured leaderboard chain ID
    async fn leaderboard_chain_id(&self) -> Option<String> {
        self.leaderboard_chain_id.map(|id| id.to_string())
    }
    
    /// Get the current session counter
    async fn session_counter(&self) -> u64 {
        self.session_counter
    }
    
    /// Get my player name
    async fn my_player_name(&self) -> &Option<String> {
        &self.my_player_name
    }
    
    /// Get all player names
    async fn all_player_names(&self) -> &Vec<PlayerNameEntry> {
        &self.all_player_names
    }
    
    /// Get player name by chain ID
    async fn player_name(&self, chain_id: String) -> Option<String> {
        self.all_player_names.iter()
            .find(|entry| entry.chain_id == chain_id)
            .map(|entry| entry.name.clone())
    }
    
    /// Get game statistics summary
    async fn game_stats(&self) -> GameStats {
        let total_sessions = self.all_sessions.len() as u64;
        let finished_games = self.all_sessions.iter().filter(|session| session.state == snake_game::GameState::Finished).count() as u64;
        let total_players = self.all_player_stats.len() as u64;
        
        GameStats {
            total_sessions,
            finished_games,
            total_players,
        }
    }
}

#[derive(async_graphql::SimpleObject)]
struct GameStats {
    total_sessions: u64,
    finished_games: u64,
    total_players: u64,
}

#[derive(async_graphql::SimpleObject)]
struct PlayerNameEntry {
    chain_id: String,
    name: String,
}

struct MutationRoot {
    runtime: Arc<ServiceRuntime<SnakeGameService>>,
}

#[Object]
impl MutationRoot {
    /// Setup the leaderboard chain (admin operation)
    async fn setup_leaderboard(&self, leaderboard_chain_id: String) -> String {
        // Parse chain ID string
        let chain_id = match leaderboard_chain_id.parse() {
            Ok(id) => id,
            Err(_) => return format!("Invalid chain ID format: {}", leaderboard_chain_id),
        };
        
        self.runtime.schedule_operation(&snake_game::Operation::SetupLeaderboard { leaderboard_chain_id: chain_id });
        format!("Setup leaderboard with chain ID: {}", leaderboard_chain_id)
    }
    
    /// Start a new game
    async fn start_game(&self) -> String {
        self.runtime.schedule_operation(&snake_game::Operation::StartGame);
        "New game started successfully".to_string()
    }
    
    /// Collect a candy during gameplay
    async fn collect_candy(&self) -> String {
        self.runtime.schedule_operation(&snake_game::Operation::CollectCandy);
        "Candy collected successfully".to_string()
    }
    
    /// End the current game
    async fn end_game(&self) -> String {
        self.runtime.schedule_operation(&snake_game::Operation::EndGame);
        "Game ended successfully".to_string()
    }
    
    /// Reset the leaderboard (admin operation, only on leaderboard chain)
    async fn reset_leaderboard(&self) -> String {
        self.runtime.schedule_operation(&snake_game::Operation::ResetLeaderboard);
        "Leaderboard reset successfully".to_string()
    }
    
    /// Set player name
    async fn set_player_name(&self, name: String) -> String {
        self.runtime.schedule_operation(&snake_game::Operation::SetPlayerName { name: name.clone() });
        format!("Player name set to '{}' successfully", name)
    }
}

#[ComplexObject]
impl SnakeGameState {}

#[ComplexObject]
impl PlayerStats {
    /// Get average candies per game as a formatted string
    async fn average_candies_formatted(&self) -> String {
        format!("{:.1}", self.average_candies())
    }
    
    /// Get current streak description
    async fn streak_description(&self) -> String {
        if self.current_streak == 0 {
            "No current streak".to_string()
        } else {
            format!("{} game win streak", self.current_streak)
        }
    }
    
    /// Get player rank based on highest score
    async fn estimated_rank(&self) -> String {
        if self.highest_score >= 100 {
            "Master".to_string()
        } else if self.highest_score >= 50 {
            "Expert".to_string()
        } else if self.highest_score >= 20 {
            "Advanced".to_string()
        } else if self.highest_score >= 5 {
            "Intermediate".to_string()
        } else {
            "Beginner".to_string()
        }
    }
}