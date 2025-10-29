// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Snake Game Cross-Chain Application */

use async_graphql::{Request, Response};
use linera_sdk::linera_base_types::{ChainId, ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct SnakeGameAbi;

impl ContractAbi for SnakeGameAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for SnakeGameAbi {
    type Query = Request;
    type QueryResponse = Response;
}

// Game state enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, async_graphql::Enum)]
pub enum GameState {
    NotStarted,
    Playing,
    Finished,
}

// Game session structure
#[derive(Debug, Clone, Serialize, Deserialize, async_graphql::SimpleObject)]
pub struct GameSession {
    pub session_id: String,
    pub player: ChainId,
    pub player_name: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub candies_collected: u32,
    pub is_record: bool,
    pub state: GameState,
}

// Leaderboard entry for global statistics
#[derive(Debug, Clone, Serialize, Deserialize, async_graphql::SimpleObject)]
pub struct LeaderboardEntry {
    pub chain_id: ChainId,
    pub player_name: Option<String>,
    pub highest_score: u32,
    pub games_played: u32,
    pub total_candies: u64,
}

// Application parameters for leaderboard configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ApplicationParameters {
    pub leaderboard_chain_id: Option<ChainId>,
}

// Cross-chain messages
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum GameMessage {
    // Request to start a game on leaderboard chain
    StartGame {
        session_id: String,
        player_chain: ChainId,
        player_name: Option<String>,
    },
    // Game finished notification with score
    GameFinished {
        session_id: String,
        player_chain: ChainId,
        candies_collected: u32,
        is_new_record: bool,
    },
    // Update leaderboard stats
    UpdateLeaderboard {
        player_chain: ChainId,
        candies_collected: u32,
        is_new_record: bool,
    },
    // Update player name on leaderboard chain
    UpdatePlayerName {
        player_chain: ChainId,
        player_name: String,
    },
    // Notification that leaderboard has been reset
    LeaderboardReset,
    // Notification that a candy was collected
    CandyCollected {
        session_id: String,
        player_chain: ChainId,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Operation {
    // Setup operations
    SetupLeaderboard {
        leaderboard_chain_id: ChainId,
    },
    
    // Player name operations
    SetPlayerName {
        name: String,
    },
    
    // Game operations
    StartGame,
    CollectCandy, // New operation to collect a candy during gameplay
    EndGame, // No longer needs candies_collected parameter
    
    // Query operations
    GetLeaderboard,
    GetMyStats,
    GetGameSession {
        session_id: String,
    },
    
    // Admin operations (only on leaderboard chain)
    ResetLeaderboard,
}