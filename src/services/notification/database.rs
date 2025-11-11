//! Database notification implementation.
//!
//! This module provides functionality to store notifications in a database
//! using SQLx with PostgreSQL support.

#[cfg(feature = "database")]
use async_trait::async_trait;
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "database")]
use serde_json::json;
#[cfg(feature = "database")]
use sqlx::{Pool, Postgres};
#[cfg(feature = "database")]
use std::{collections::HashMap, sync::Arc};

#[cfg(feature = "database")]
use crate::{
	models::{MatchConditions, MonitorMatch, TriggerTypeConfig},
	services::notification::error::NotificationError,
};

#[cfg(feature = "database")]
/// Configuration for database notification storage
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
	/// Database connection string
	pub connection_string: String,
	/// Table name to store notifications
	pub table_name: String,
	/// Optional additional fields to store
	pub additional_fields: Option<HashMap<String, String>>,
}

#[cfg(feature = "database")]
/// Simplified notification record that stores only decoded/processed data
/// This struct is optimized to avoid storing raw transaction bytes and other bloated data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
	/// Transaction hash
	pub transaction_hash: String,
	/// Block number (if available)
	pub block_number: Option<i64>,
	/// Network slug
	pub network: String,
	/// Monitor name
	pub monitor_name: String,
	/// Matched conditions (functions, events, transactions)
	pub matched_conditions: MatchConditions,
	/// Decoded arguments from matched conditions
	pub decoded_args: Option<serde_json::Value>,
}

#[cfg(feature = "database")]
impl NotificationRecord {
	/// Creates a notification record from a monitor match
	/// Extracts only the essential decoded/processed data
	pub fn from_monitor_match(monitor_match: &MonitorMatch) -> Result<Self, NotificationError> {
		match monitor_match {
			MonitorMatch::EVM(evm_match) => {
				let block_number = evm_match
					.transaction
					.block_number
					.map(|n| n.to::<i64>());
				
				let decoded_args = evm_match.matched_on_args.as_ref().map(|args| {
					serde_json::to_value(args).unwrap_or(serde_json::Value::Null)
				});

				Ok(NotificationRecord {
					transaction_hash: format!("{:#x}", evm_match.transaction.hash),
					block_number,
					network: evm_match.network_slug.clone(),
					monitor_name: evm_match.monitor.name.clone(),
					matched_conditions: evm_match.matched_on.clone(),
					decoded_args,
				})
			}
			MonitorMatch::Stellar(stellar_match) => {
				let decoded_args = stellar_match.matched_on_args.as_ref().map(|args| {
					serde_json::to_value(args).unwrap_or(serde_json::Value::Null)
				});

				Ok(NotificationRecord {
					transaction_hash: stellar_match.transaction.hash().clone(),
					block_number: Some(stellar_match.ledger.sequence as i64),
					network: stellar_match.network_slug.clone(),
					monitor_name: stellar_match.monitor.name.clone(),
					matched_conditions: stellar_match.matched_on.clone(),
					decoded_args,
				})
			}
			MonitorMatch::Midnight(midnight_match) => {
				let decoded_args = midnight_match.matched_on_args.as_ref().map(|args| {
					serde_json::to_value(args).unwrap_or(serde_json::Value::Null)
				});

				Ok(NotificationRecord {
					transaction_hash: midnight_match.transaction.hash().clone(),
					block_number: None,
					network: midnight_match.network_slug.clone(),
					monitor_name: midnight_match.monitor.name.clone(),
					matched_conditions: midnight_match.matched_on.clone(),
					decoded_args,
				})
			}
		}
	}
}

#[cfg(feature = "database")]
impl DatabaseConfig {
	/// Creates a new database configuration from trigger config
	pub fn from_trigger_config(config: &TriggerTypeConfig) -> Result<Self, NotificationError> {
		match config {
			TriggerTypeConfig::Database {
				connection_string,
				table_name,
				additional_fields,
			} => Ok(DatabaseConfig {
				connection_string: connection_string.as_ref().to_string(),
				table_name: table_name.clone(),
				additional_fields: additional_fields.clone(),
			}),
			_ => Err(NotificationError::config_error(
				"Invalid database configuration".to_string(),
				None,
				None,
			)),
		}
	}
}

#[cfg(feature = "database")]
/// Database notifier for storing notifications
pub struct DatabaseNotifier {
	config: DatabaseConfig,
	pool: Arc<Pool<Postgres>>,
}

#[cfg(feature = "database")]
impl DatabaseNotifier {
	/// Creates a new database notifier instance
	pub fn new(config: DatabaseConfig, pool: Arc<Pool<Postgres>>) -> Self {
		DatabaseNotifier { config, pool }
	}

	/// Creates notifier from trigger configuration
	pub fn from_config(
		config: &TriggerTypeConfig,
		pool: Arc<Pool<Postgres>>,
	) -> Result<Self, NotificationError> {
		let db_config = DatabaseConfig::from_trigger_config(config)?;
		Ok(Self::new(db_config, pool))
	}

	/// Stores notification in database
	///
	/// # Arguments
	/// * `monitor_match` - The monitor match to store
	/// * `variables` - Variables to include in the notification
	///
	/// # Returns
	/// * `Result<(), NotificationError>` - Success or error
	pub async fn notify(
		&self,
		monitor_match: &MonitorMatch,
		variables: &HashMap<String, String>,
	) -> Result<(), NotificationError> {
		// Create simplified notification record with only decoded/processed data
		let record = NotificationRecord::from_monitor_match(monitor_match)?;

		// Serialize the record fields
		let matched_conditions_json = serde_json::to_value(&record.matched_conditions).map_err(|e| {
			NotificationError::execution_error(
				"Failed to serialize matched conditions".to_string(),
				Some(e.into()),
				None,
			)
		})?;

		let decoded_args_json = record.decoded_args.unwrap_or(serde_json::Value::Null);

		// Prepare additional fields JSON
		let additional_fields_json = match &self.config.additional_fields {
			Some(fields) => json!(fields),
			None => json!({}),
		};

		// Variables as JSON
		let variables_json = json!(variables);

		// Insert notification into database with only essential data
		let query = format!(
			"INSERT INTO {} (transaction_hash, block_number, network, monitor_name, matched_conditions, decoded_args, variables, additional_fields, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())",
			self.config.table_name
		);

		sqlx::query(&query)
			.bind(&record.transaction_hash)
			.bind(record.block_number)
			.bind(&record.network)
			.bind(&record.monitor_name)
			.bind(matched_conditions_json)
			.bind(decoded_args_json)
			.bind(variables_json)
			.bind(additional_fields_json)
			.execute(&*self.pool)
			.await
			.map_err(|e| {
				NotificationError::execution_error(
					format!("Failed to store notification in database: {}", e),
					Some(e.into()),
					None,
				)
			})?;

		tracing::info!(
			"Successfully stored notification in database table: {} for tx: {}",
			self.config.table_name,
			record.transaction_hash
		);

		Ok(())
	}
}

#[cfg(feature = "database")]
/// Trait for database notification execution
#[async_trait]
pub trait DatabaseNotificationExecutor {
	/// Stores a notification in the database
	///
	/// # Arguments
	/// * `monitor_match` - The monitor match to store
	/// * `variables` - Variables to include in the notification
	///
	/// # Returns
	/// * `Result<(), NotificationError>` - Success or error
	async fn database_notify(
		&self,
		monitor_match: &MonitorMatch,
		variables: &HashMap<String, String>,
	) -> Result<(), NotificationError>;
}

#[cfg(feature = "database")]
#[async_trait]
impl DatabaseNotificationExecutor for DatabaseNotifier {
	async fn database_notify(
		&self,
		monitor_match: &MonitorMatch,
		variables: &HashMap<String, String>,
	) -> Result<(), NotificationError> {
		self.notify(monitor_match, variables).await
	}
}

#[cfg(test)]
#[cfg(feature = "database")]
mod tests {
	use super::*;
	use crate::{
		models::{
			AddressWithSpec, EVMMonitorMatch, EVMTransactionReceipt, EventCondition,
			FunctionCondition, MatchConditions, Monitor, MonitorMatch, TransactionCondition,
		},
		utils::tests::{
			builders::evm::monitor::MonitorBuilder, evm::transaction::TransactionBuilder,
		},
	};
	use alloy::primitives::{B256, U64};

	fn create_test_monitor(
		event_conditions: Vec<EventCondition>,
		function_conditions: Vec<FunctionCondition>,
		transaction_conditions: Vec<TransactionCondition>,
		addresses: Vec<AddressWithSpec>,
	) -> Monitor {
		let mut builder = MonitorBuilder::new()
			.name("test")
			.networks(vec!["evm_mainnet".to_string()]);

		for event in event_conditions {
			builder = builder.event(&event.signature, event.expression);
		}
		for function in function_conditions {
			builder = builder.function(&function.signature, function.expression);
		}
		for transaction in transaction_conditions {
			builder = builder.transaction(transaction.status, transaction.expression);
		}

		for addr in addresses {
			builder = builder.address(&addr.address);
		}

		builder.build()
	}

	fn create_mock_monitor_match() -> MonitorMatch {
		let mut tx = TransactionBuilder::new()
			.hash(B256::with_last_byte(1))
			.build();
		
		// Set block number directly on the transaction
		tx.0.block_number = Some(U64::from(12345));

		MonitorMatch::EVM(Box::new(EVMMonitorMatch {
			monitor: create_test_monitor(vec![], vec![], vec![], vec![]),
			transaction: tx,
			receipt: Some(EVMTransactionReceipt::default()),
			logs: Some(vec![]),
			network_slug: "evm_mainnet".to_string(),
			matched_on: MatchConditions {
				functions: vec![],
				events: vec![],
				transactions: vec![],
			},
			matched_on_args: None,
		}))
	}

	#[test]
	fn test_notification_record_from_evm_match() {
		let monitor_match = create_mock_monitor_match();
		let record = NotificationRecord::from_monitor_match(&monitor_match).unwrap();

		assert_eq!(record.monitor_name, "test");
		assert_eq!(record.network, "evm_mainnet");
		assert!(record.transaction_hash.starts_with("0x"));
		assert_eq!(record.block_number, Some(12345));
		assert!(record.decoded_args.is_none());
	}

	#[test]
	fn test_notification_record_serialization() {
		let monitor_match = create_mock_monitor_match();
		let record = NotificationRecord::from_monitor_match(&monitor_match).unwrap();

		// Ensure the record can be serialized
		let json = serde_json::to_value(&record).unwrap();
		assert!(json.is_object());
		assert!(json.get("transaction_hash").is_some());
		assert!(json.get("monitor_name").is_some());
		assert!(json.get("network").is_some());
		assert!(json.get("matched_conditions").is_some());
	}

	#[test]
	fn test_database_config_from_trigger() {
		use crate::models::SecretValue;

		let config = TriggerTypeConfig::Database {
			connection_string: SecretValue::Plain(crate::models::SecretString::new(
				"postgres://localhost/test".to_string(),
			)),
			table_name: "notifications".to_string(),
			additional_fields: None,
		};

		let db_config = DatabaseConfig::from_trigger_config(&config).unwrap();
		assert_eq!(db_config.table_name, "notifications");
		assert!(db_config.additional_fields.is_none());
	}

	#[test]
	fn test_database_config_from_invalid_trigger() {
		use crate::models::{NotificationMessage, SecretValue};

		let config = TriggerTypeConfig::Slack {
			slack_url: SecretValue::Plain(crate::models::SecretString::new(
				"https://hooks.slack.com/test".to_string(),
			)),
			message: NotificationMessage {
				title: "Test".to_string(),
				body: "Test".to_string(),
			},
			retry_policy: Default::default(),
		};

		let result = DatabaseConfig::from_trigger_config(&config);
		assert!(result.is_err());
	}
}
