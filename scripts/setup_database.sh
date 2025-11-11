#!/usr/bin/env bash
# Database setup script for OpenZeppelin Monitor
# This script sets up the PostgreSQL database for database notification triggers

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DB_SCHEMA_FILE="${SCRIPT_DIR}/db/schema.sql"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default connection parameters (can be overridden by environment variables)
PGHOST="${PGHOST:-localhost}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-monitor}"
PGPASSWORD="${PGPASSWORD:-monitor_password}"
PGDATABASE="${PGDATABASE:-monitor_notifications}"

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Setup PostgreSQL database for OpenZeppelin Monitor notifications.

OPTIONS:
    -h, --help              Show this help message
    -c, --create-db         Create the database if it doesn't exist
    -d, --drop              Drop and recreate the notifications table (DESTRUCTIVE!)
    -m, --migrate           Apply schema to existing database
    --docker                Use docker exec to run commands in postgres container
    --container NAME        Container name (default: openzeppelin-monitor-postgres)

ENVIRONMENT VARIABLES:
    PGHOST                  Database host (default: localhost)
    PGPORT                  Database port (default: 5432)
    PGUSER                  Database user (default: monitor)
    PGPASSWORD              Database password (default: monitor_password)
    PGDATABASE              Database name (default: monitor_notifications)

EXAMPLES:
    # Setup using local psql
    $0 --migrate

    # Setup using docker container
    $0 --docker --migrate

    # Drop and recreate (CAUTION: destroys data!)
    $0 --docker --drop

    # Create database and apply schema
    $0 --create-db --migrate

EOF
}

# Parse command line arguments
CREATE_DB=false
DROP_TABLE=false
MIGRATE=false
USE_DOCKER=false
CONTAINER_NAME="openzeppelin-monitor-postgres"

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            print_usage
            exit 0
            ;;
        -c|--create-db)
            CREATE_DB=true
            shift
            ;;
        -d|--drop)
            DROP_TABLE=true
            shift
            ;;
        -m|--migrate)
            MIGRATE=true
            shift
            ;;
        --docker)
            USE_DOCKER=true
            shift
            ;;
        --container)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

# Function to run psql command
run_psql() {
    local cmd="$1"
    if [ "$USE_DOCKER" = true ]; then
        docker exec -i "$CONTAINER_NAME" psql -U "$PGUSER" -d "$PGDATABASE" -c "$cmd"
    else
        PGPASSWORD="$PGPASSWORD" psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -c "$cmd"
    fi
}

# Function to run psql with file input
run_psql_file() {
    local file="$1"
    if [ "$USE_DOCKER" = true ]; then
        docker exec -i "$CONTAINER_NAME" psql -U "$PGUSER" -d "$PGDATABASE" < "$file"
    else
        PGPASSWORD="$PGPASSWORD" psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -f "$file"
    fi
}

# Check if schema file exists
if [ ! -f "$DB_SCHEMA_FILE" ]; then
    echo -e "${RED}Error: Schema file not found: $DB_SCHEMA_FILE${NC}"
    exit 1
fi

# Create database if requested
if [ "$CREATE_DB" = true ]; then
    echo -e "${YELLOW}Creating database '$PGDATABASE'...${NC}"
    if [ "$USE_DOCKER" = true ]; then
        docker exec -i "$CONTAINER_NAME" psql -U "$PGUSER" -d postgres -c "CREATE DATABASE $PGDATABASE;" 2>/dev/null || true
    else
        PGPASSWORD="$PGPASSWORD" psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d postgres -c "CREATE DATABASE $PGDATABASE;" 2>/dev/null || true
    fi
    echo -e "${GREEN}Database created (or already exists)${NC}"
fi

# Drop table if requested
if [ "$DROP_TABLE" = true ]; then
    echo -e "${YELLOW}WARNING: About to drop monitor_notifications table!${NC}"
    read -p "Are you sure? (type 'yes' to confirm): " confirm
    if [ "$confirm" != "yes" ]; then
        echo -e "${RED}Aborted.${NC}"
        exit 1
    fi
    echo -e "${YELLOW}Dropping monitor_notifications table...${NC}"
    run_psql "DROP TABLE IF EXISTS monitor_notifications CASCADE;"
    echo -e "${GREEN}Table dropped${NC}"
fi

# Apply schema
if [ "$MIGRATE" = true ] || [ "$DROP_TABLE" = true ]; then
    echo -e "${YELLOW}Applying schema from $DB_SCHEMA_FILE...${NC}"
    run_psql_file "$DB_SCHEMA_FILE"
    echo -e "${GREEN}Schema applied successfully${NC}"
    
    # Verify
    echo -e "${YELLOW}Verifying table structure...${NC}"
    run_psql "\d monitor_notifications"
fi

echo -e "${GREEN}Database setup complete!${NC}"
echo -e "${YELLOW}Connection details:${NC}"
echo -e "  Host:     $PGHOST"
echo -e "  Port:     $PGPORT"
echo -e "  Database: $PGDATABASE"
echo -e "  User:     $PGUSER"
