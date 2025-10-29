// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext, SetView};
use linera_sdk::linera_base_types::ChainId;
use serde::{Deserialize, Serialize};
use async_graphql::SimpleObject;
use snake_game::{GameSession, LeaderboardEntry};

/// Player statistics for tracking personal game history
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PlayerStats {
    pub chain_id: ChainId,
    pub games_played: u32,
    pub highest_score: u32,
    pub total_candies: u64,
    pub current_streak: u32,
    pub best_streak: u32,
    pub last_game_timestamp: u64,
}

impl PlayerStats {
    #[allow(dead_code)]
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            games_played: 0,
            highest_score: 0,
            total_candies: 0,
            current_streak: 0,
            best_streak: 0,
            last_game_timestamp: 0,
        }
    }
    
    #[allow(dead_code)]
    pub fn add_game(&mut self, candies_collected: u32, timestamp: u64) -> bool {
        self.games_played += 1;
        self.total_candies += candies_collected as u64;
        self.last_game_timestamp = timestamp;
        
        let is_record = candies_collected > self.highest_score;
        if is_record {
            self.highest_score = candies_collected;
            self.current_streak += 1;
            if self.current_streak > self.best_streak {
                self.best_streak = self.current_streak;
            }
        } else {
            self.current_streak = 0;
        }
        
        is_record
    }
    
    pub fn average_candies(&self) -> f64 {
        if self.games_played > 0 {
            (self.total_candies as f64) / (self.games_played as f64)
        } else {
            0.0
        }
    }
}

/// The application state for Snake Game
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct SnakeGameState {
    // Game state
    pub sessions: MapView<String, GameSession>, // session_id -> GameSession
    pub session_counter: RegisterView<u64>, // Counter for generating unique session IDs
    
    // Player names
    pub player_names: MapView<ChainId, String>, // chain_id -> player_name
    pub my_player_name: RegisterView<Option<String>>, // This player's name
    
    // Leaderboard state (only on leaderboard chain)
    pub global_leaderboard: RegisterView<Vec<LeaderboardEntry>>, // Top players globally
    pub player_stats: MapView<ChainId, PlayerStats>, // chain_id -> detailed stats
    pub leaderboard_participants: SetView<ChainId>, // Tracks which chains have been in the leaderboard
    pub is_leaderboard_chain: RegisterView<bool>, // Flag to identify if this is the leaderboard chain
    pub leaderboard_chain_id: RegisterView<Option<ChainId>>, // Store the leaderboard chain ID
    
    // Player-specific state (on each player's chain)
    pub my_sessions: RegisterView<Vec<String>>, // Sessions this player participated in
    pub my_stats: RegisterView<Option<PlayerStats>>, // Personal statistics
    pub my_current_session: RegisterView<Option<String>>, // Currently active session
}