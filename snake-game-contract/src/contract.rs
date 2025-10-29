// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use snake_game::{ApplicationParameters, GameMessage, Operation, SnakeGameAbi, 
    GameSession, LeaderboardEntry, GameState};
use linera_sdk::{
    linera_base_types::{ChainId, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use async_graphql::ComplexObject;

use self::state::{SnakeGameState, PlayerStats};

linera_sdk::contract!(SnakeGameContract);

pub struct SnakeGameContract {
    state: SnakeGameState,
    runtime: ContractRuntime<Self>,
}

impl WithContractAbi for SnakeGameContract {
    type Abi = SnakeGameAbi;
}

impl Contract for SnakeGameContract {
    type Message = GameMessage;
    type InstantiationArgument = ();
    type Parameters = ApplicationParameters;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = SnakeGameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        SnakeGameContract { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        // Validate that the application parameters were configured correctly.
        let parameters = self.runtime.application_parameters();
        
        // Initialize game state
        self.state.session_counter.set(0);
        
        // Initialize leaderboard state
        self.state.global_leaderboard.set(Vec::new());
        self.state.leaderboard_chain_id.set(parameters.leaderboard_chain_id);
        
        // Check if this chain is the leaderboard chain
        let is_leaderboard = parameters.leaderboard_chain_id
            .map(|chain_id| chain_id == self.runtime.chain_id())
            .unwrap_or(false);
        self.state.is_leaderboard_chain.set(is_leaderboard);
        
        // Initialize player-specific state
        self.state.my_sessions.set(Vec::new());
        self.state.my_stats.set(None);
        self.state.my_current_session.set(None);
        
        eprintln!("[INIT] Snake Game contract initialized on chain {:?}", self.runtime.chain_id());
        eprintln!("[INIT] Is leaderboard chain: {}", is_leaderboard);
        eprintln!("[INIT] Configured leaderboard chain: {:?}", parameters.leaderboard_chain_id);
    }

    async fn execute_operation(&mut self, operation: Operation) -> () {
        match operation {
            Operation::SetupLeaderboard { leaderboard_chain_id } => {
                eprintln!("[SETUP] SetupLeaderboard called on chain {:?} with leaderboard_chain_id: {:?}", 
                    self.runtime.chain_id(), leaderboard_chain_id);
                
                // Only allow setup if not already configured
                if self.state.leaderboard_chain_id.get().is_some() {
                    panic!("Leaderboard already configured");
                }

                // Set the leaderboard chain ID
                self.state.leaderboard_chain_id.set(Some(leaderboard_chain_id));

                // If this chain is being designated as the leaderboard chain
                if self.runtime.chain_id() == leaderboard_chain_id {
                    self.state.is_leaderboard_chain.set(true);
                    eprintln!("[SETUP] This chain IS the leaderboard chain");
                } else {
                    eprintln!("[SETUP] This chain is NOT the leaderboard chain");
                }
            }
            
            Operation::SetPlayerName { name } => {
                let current_chain = self.runtime.chain_id();
                eprintln!("[SET_NAME] Setting player name '{}' for chain {:?}", name, current_chain);
                
                // Set the player name locally
                self.state.my_player_name.set(Some(name.clone()));
                
                // Send name update to leaderboard chain if this is not the leaderboard chain
                if let Some(leaderboard_chain_id) = *self.state.leaderboard_chain_id.get() {
                    if current_chain != leaderboard_chain_id {
                        let message = GameMessage::UpdatePlayerName {
                            player_chain: current_chain,
                            player_name: name,
                        };
                        self.runtime.send_message(leaderboard_chain_id, message);
                    } else {
                        // If this is the leaderboard chain, update the name mapping directly
                        let _ = self.state.player_names.insert(&current_chain, name);
                    }
                }
            }
            
            Operation::StartGame => {
                let current_chain = self.runtime.chain_id();
                let player_name = self.state.my_player_name.get().clone();
                let timestamp = self.runtime.system_time().micros();
                
                // Generate unique session ID
                let session_counter = *self.state.session_counter.get();
                let session_id = format!("session_{}_{}", current_chain, session_counter);
                self.state.session_counter.set(session_counter + 1);
                
                // Create local game session (only stored on player's chain)
                let session = GameSession {
                    session_id: session_id.clone(),
                    player: current_chain,
                    player_name,
                    start_time: timestamp,
                    end_time: None,
                    candies_collected: 0, // Start with 0 candies
                    is_record: false,
                    state: GameState::Playing,
                };
                
                let _ = self.state.sessions.insert(&session_id, session);
                
                // Add session to player's session list
                let mut my_sessions = self.state.my_sessions.get().clone();
                my_sessions.push(session_id.clone());
                self.state.my_sessions.set(my_sessions);
                
                // Set as current session
                self.state.my_current_session.set(Some(session_id.clone()));
                
                eprintln!("[START_GAME] Started new game session: {} on player chain {:?}", session_id, current_chain);
            }
            
            Operation::CollectCandy => {
                let current_chain = self.runtime.chain_id();
                let leaderboard_chain = self.state.leaderboard_chain_id.get().clone();
                
                // Get current session
                if let Some(session_id) = self.state.my_current_session.get().clone() {
                    // Update local session to increment candy count
                    if let Ok(Some(mut session)) = self.state.sessions.get(&session_id).await {
                        session.candies_collected += 1;
                        let candies_collected = session.candies_collected; // Store the value before moving the session
                        let _ = self.state.sessions.insert(&session_id, session);
                        
                        // Send CandyCollected message to leaderboard chain
                        match leaderboard_chain {
                            Some(leader_chain) => {
                                let message = GameMessage::CandyCollected {
                                    session_id: session_id.clone(),
                                    player_chain: current_chain,
                                };
                                self.runtime.send_message(leader_chain, message);
                                eprintln!("[COLLECT_CANDY] Sent CandyCollected to leaderboard chain {:?} for session {} (total: {})", 
                                    leader_chain, session_id, candies_collected);
                            }
                            None => {
                                eprintln!("[ERROR] No leaderboard chain configured for collecting candy. Please use SetupLeaderboard operation first");
                            }
                        }
                        
                        eprintln!("[COLLECT_CANDY] Collected candy in session: {} (total: {})", 
                            session_id, candies_collected);
                    }
                } else {
                    eprintln!("[ERROR] No active game session found for collecting candy");
                }
            }
            
            Operation::EndGame => {
                let current_chain = self.runtime.chain_id();
                let leaderboard_chain = self.state.leaderboard_chain_id.get().clone();
                let timestamp = self.runtime.system_time().micros();
                
                // Get current session
                if let Some(session_id) = self.state.my_current_session.get().clone() {
                    // Get the session data (we don't need to modify it here)
                    if let Ok(Some(session)) = self.state.sessions.get(&session_id).await {
                        let candies_collected = session.candies_collected;
                        
                        // Update session to mark as finished
                        let mut updated_session = session.clone();
                        updated_session.end_time = Some(timestamp);
                        updated_session.state = GameState::Finished;
                        
                        // Check if this is a new record for this player
                        let is_new_record = if let Some(ref stats) = *self.state.my_stats.get() {
                            candies_collected > stats.highest_score
                        } else {
                            true // First game is always a record
                        };
                        
                        updated_session.is_record = is_new_record;
                        let _ = self.state.sessions.insert(&session_id, updated_session);
                        
                        // Only send GameFinished message to leaderboard chain if it's a new record
                        if is_new_record {
                            match leaderboard_chain {
                                Some(leader_chain) => {
                                    let message = GameMessage::GameFinished {
                                        session_id: session_id.clone(),
                                        player_chain: current_chain,
                                        candies_collected,
                                        is_new_record,
                                    };
                                    self.runtime.send_message(leader_chain, message);
                                    eprintln!("[END_GAME] Sent GameFinished to leaderboard chain {:?} with {} candies (new record: {})", 
                                        leader_chain, candies_collected, is_new_record);
                                }
                                None => {
                                    eprintln!("[ERROR] No leaderboard chain configured for ending game. Please use SetupLeaderboard operation first");
                                }
                            }
                        } else {
                            eprintln!("[END_GAME] Game ended with {} candies, but not a new record. Skipping leaderboard update.", 
                                candies_collected);
                        }
                        
                        // Update personal stats
                        let mut my_stats = self.state.my_stats.get().clone().unwrap_or_else(|| PlayerStats::new(current_chain));
                        my_stats.add_game(candies_collected, timestamp);
                        self.state.my_stats.set(Some(my_stats));
                        
                        // Clear current session
                        self.state.my_current_session.set(None);
                        
                        eprintln!("[END_GAME] Ended game session: {} with {} candies (record: {})", 
                            session_id, candies_collected, is_new_record);
                    }
                } else {
                    eprintln!("[ERROR] No active game session found");
                }
            }
            
            Operation::GetLeaderboard => {
                // This operation doesn't modify state, just allows querying leaderboard
                // The actual leaderboard can be queried through the service
            }
            
            Operation::GetMyStats => {
                // This operation doesn't modify state, just allows querying personal stats
                // The actual stats can be queried through the service
            }
            
            Operation::GetGameSession { session_id: _ } => {
                // This operation doesn't modify state, just allows querying specific session
                // The actual session can be queried through the service
            }
            
            Operation::ResetLeaderboard => {
                eprintln!("[RESET] ResetLeaderboard called on chain {:?}", self.runtime.chain_id());
                
                // Only allow reset on the leaderboard chain
                if !*self.state.is_leaderboard_chain.get() {
                    panic!("Reset operation can only be performed on the leaderboard chain");
                }
                
                // Get the list of players who were in the leaderboard before clearing
                let mut leaderboard_players = Vec::new();
                match self.state.leaderboard_participants.indices().await {
                    Ok(players) => {
                        for player in players {
                            leaderboard_players.push(player);
                        }
                        eprintln!("[RESET] Found {} players who were in the leaderboard", leaderboard_players.len());
                    }
                    Err(e) => {
                        eprintln!("[RESET] Error getting leaderboard participants: {:?}", e);
                    }
                }
                
                // Clear all game data on leaderboard chain
                self.state.global_leaderboard.set(Vec::new());
                self.state.player_stats.clear();
                self.state.leaderboard_participants.clear();
                self.state.session_counter.set(0);
                
                // Send LeaderboardReset message to all players who were in the leaderboard
                for player_chain in &leaderboard_players {
                    if *player_chain != self.runtime.chain_id() {
                        let message = GameMessage::LeaderboardReset;
                        self.runtime.send_message(*player_chain, message);
                        eprintln!("[RESET] Sent LeaderboardReset message to player chain {:?}", player_chain);
                    }
                }
                
                eprintln!("[RESET] Leaderboard reset completed successfully on leaderboard chain");
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        eprintln!("[MESSAGE] Received message on chain {:?}", self.runtime.chain_id());
        
        // Check if message is bouncing
        let is_bouncing = self
            .runtime
            .message_is_bouncing()
            .expect("Message delivery status must be available when executing a message");

        if is_bouncing {
            eprintln!("[MESSAGE] Message is bouncing, returning");
            return;
        }

        match message {
            GameMessage::StartGame { .. } => {
                // Ignore StartGame messages on all chains as sessions are only stored locally
                eprintln!("[MESSAGE] Ignoring StartGame message - sessions are stored locally only");
            }
            
            GameMessage::CandyCollected { session_id: _, player_chain } => {
                eprintln!("[MESSAGE] Processing CandyCollected from player chain {:?}", player_chain);
                
                // Only process on leaderboard chain
                if !*self.state.is_leaderboard_chain.get() {
                    eprintln!("[MESSAGE] This is NOT the leaderboard chain, ignoring CandyCollected message");
                    return;
                }
                
                // In a real implementation, we might want to track candy collection on the leaderboard chain
                // For now, we'll just log the event
                eprintln!("[MESSAGE] Player chain {:?} collected a candy", player_chain);
            }
            
            GameMessage::GameFinished { session_id: _, player_chain, candies_collected, is_new_record } => {
                eprintln!("[MESSAGE] Processing GameFinished: from {:?} with {} candies (new record: {})", 
                    player_chain, candies_collected, is_new_record);
                
                // Only process on leaderboard chain
                if !*self.state.is_leaderboard_chain.get() {
                    eprintln!("[MESSAGE] This is NOT the leaderboard chain, ignoring GameFinished message");
                    return;
                }
                
                // Update leaderboard stats only (no session tracking on leaderboard chain)
                self.update_leaderboard_stats(player_chain, candies_collected, is_new_record).await;
            }
            
            GameMessage::UpdateLeaderboard { player_chain, candies_collected, is_new_record } => {
                eprintln!("[MESSAGE] Processing UpdateLeaderboard for {:?}, candies: {}, new record: {}", 
                    player_chain, candies_collected, is_new_record);
                
                // Only process on leaderboard chain
                if !*self.state.is_leaderboard_chain.get() {
                    eprintln!("[MESSAGE] This is NOT the leaderboard chain, ignoring UpdateLeaderboard message");
                    return;
                }
                
                self.update_leaderboard_stats(player_chain, candies_collected, is_new_record).await;
            }
            
            GameMessage::UpdatePlayerName { player_chain, player_name } => {
                eprintln!("[MESSAGE] Processing UpdatePlayerName for {:?}: '{}'", player_chain, player_name);
                
                // Only process on leaderboard chain
                if !*self.state.is_leaderboard_chain.get() {
                    eprintln!("[MESSAGE] This is NOT the leaderboard chain, ignoring UpdatePlayerName message");
                    return;
                }
                
                // Store the player name mapping
                let _ = self.state.player_names.insert(&player_chain, player_name);
                eprintln!("[MESSAGE] Updated player name for chain {:?}", player_chain);
            }
            
            GameMessage::LeaderboardReset => {
                eprintln!("[MESSAGE] Processing LeaderboardReset notification on chain {:?}", self.runtime.chain_id());
                
                // Clear local leaderboard data on player chains
                // On the leaderboard chain, this would be redundant, but we'll handle it gracefully
                if *self.state.is_leaderboard_chain.get() {
                    eprintln!("[MESSAGE] This is the leaderboard chain, ignoring LeaderboardReset message");
                    return;
                }
                
                // Clear local player stats when leaderboard is reset
                // This will reset the highest score and all other player statistics
                if let Some(mut stats) = self.state.my_stats.get().clone() {
                    stats.highest_score = 0;
                    stats.games_played = 0;
                    stats.total_candies = 0;
                    stats.current_streak = 0;
                    stats.best_streak = 0;
                    self.state.my_stats.set(Some(stats));
                    eprintln!("[MESSAGE] Player chain {:?} cleared local stats due to leaderboard reset", 
                        self.runtime.chain_id());
                } else {
                    eprintln!("[MESSAGE] Player chain {:?} had no local stats to clear", 
                        self.runtime.chain_id());
                }
                
                // Also clear the global leaderboard on this player chain if it exists
                self.state.global_leaderboard.set(Vec::new());
                eprintln!("[MESSAGE] Player chain {:?} cleared local leaderboard data", 
                    self.runtime.chain_id());
            }
        }
    }

    async fn store(mut self) {
        let _ = self.state.save().await;
    }
}

impl SnakeGameContract {
    async fn update_leaderboard_stats(&mut self, player_chain: ChainId, candies_collected: u32, is_new_record: bool) {
        eprintln!("[LEADERBOARD] Updating stats for {:?}, candies: {}, new record: {}", 
            player_chain, candies_collected, is_new_record);
        
        let timestamp = self.runtime.system_time().micros();
        
        // Get or create player stats
        let mut stats = match self.state.player_stats.get(&player_chain).await {
            Ok(Some(existing_stats)) => existing_stats,
            _ => PlayerStats::new(player_chain),
        };
        
        // Update stats
        let _was_record = stats.add_game(candies_collected, timestamp); // Prefix with underscore to indicate intentional omission
        
        // Save updated stats
        let _ = self.state.player_stats.insert(&player_chain, stats.clone());
        
        // Add player to leaderboard participants set
        let _ = self.state.leaderboard_participants.insert(&player_chain);
        
        // Rebuild global leaderboard
        self.rebuild_global_leaderboard().await;
        
        eprintln!("[LEADERBOARD] Updated stats for {:?}: games={}, highest={}, total_candies={}, avg={:.2}", 
            player_chain, stats.games_played, stats.highest_score, stats.total_candies, stats.average_candies());
    }
    
    /// Rebuild the global leaderboard from all player stats
    async fn rebuild_global_leaderboard(&mut self) {
        // Collect all player stats
        let mut all_entries = Vec::new();

        // Get all player chain IDs who have stats
        match self.state.player_stats.indices().await {
            Ok(player_chains) => {
                eprintln!("[LEADERBOARD] Found {} players with stats", player_chains.len());

                for player_chain in player_chains {
                    if let Ok(Some(stats)) = self.state.player_stats.get(&player_chain).await {
                        // Get player name if available
                        let player_name = match self.state.player_names.get(&player_chain).await {
                            Ok(Some(name)) => Some(name),
                            _ => None,
                        };
                        
                        let entry = LeaderboardEntry {
                            chain_id: stats.chain_id,
                            highest_score: stats.highest_score,
                            games_played: stats.games_played,
                            total_candies: stats.total_candies,
                            player_name: player_name.clone(),
                        };
                        all_entries.push(entry);
                        eprintln!("[LEADERBOARD] Added {:?} ({:?}) with {} highest score to rebuild list", 
                            player_chain, player_name, stats.highest_score);
                    }
                }
            }
            Err(_) => {
                eprintln!("[LEADERBOARD] Failed to get player chains, returning");
                return;
            }
        }

        // Sort by highest score descending, then by total candies, then by games played
        all_entries.sort_by(|a, b| {
            b.highest_score.cmp(&a.highest_score)
                .then_with(|| b.total_candies.cmp(&a.total_candies))
                .then_with(|| b.games_played.cmp(&a.games_played))
        });
        eprintln!("[LEADERBOARD] Sorted {} entries", all_entries.len());

        // Take top 100
        let top_100: Vec<LeaderboardEntry> = all_entries.into_iter().take(100).collect();
        eprintln!("[LEADERBOARD] Taking top {} entries for leaderboard", top_100.len());

        // Update the global leaderboard
        self.state.global_leaderboard.set(top_100.clone());
        eprintln!("[LEADERBOARD] Global leaderboard updated with {} entries", top_100.len());
        
        // Log final leaderboard state
        eprintln!("[LEADERBOARD] Final leaderboard state:");
        for (i, entry) in top_100.iter().take(10).enumerate() {
            let display_name = entry.player_name.as_ref().map(|s| s.as_str()).unwrap_or("Anonymous");
            eprintln!("[LEADERBOARD] #{}: {} ({:?}) - {} highest score, {} total candies ({} games)", 
                i + 1, display_name, entry.chain_id, entry.highest_score, entry.total_candies, entry.games_played);
        }
        
        eprintln!("[LEADERBOARD] Rebuild completed successfully");
    }
}

#[ComplexObject]
impl SnakeGameState {}