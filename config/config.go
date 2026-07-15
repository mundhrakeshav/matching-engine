package config

import (
	"fmt"
	"os"

	"github.com/go-playground/validator/v10"
	"github.com/joho/godotenv"
	"github.com/kelseyhightower/envconfig"
)

type Config struct {
	Server ServerConfig
	DB     DatabaseConfig
}

type ServerConfig struct {
	Port        int    `envconfig:"PORT" default:"8080" validate:"required,min=1,max=65535"`
	Host        string `envconfig:"HOST" validate:"required"`
	ServiceName string `envconfig:"SERVICE_NAME" validate:"required"`
	LogLevel    string `envconfig:"LOG_LEVEL" default:"info" validate:"required,oneof=debug info warn error"`
}

type DatabaseConfig struct {
	Host     string `envconfig:"DB_HOST" validate:"required"`
	Port     int    `envconfig:"DB_PORT" default:"5432" validate:"required,min=1,max=65535"`
	User     string `envconfig:"DB_USER" validate:"required"`
	Password string `envconfig:"DB_PASSWORD" validate:"required"`
	Name     string `envconfig:"DB_NAME" validate:"required"`
	SSLMode  string `envconfig:"DB_SSLMODE" default:"disable" validate:"required,oneof=disable require verify-ca verify-full"`
}

func Load() (*Config, error) {
	if os.Getenv("ENV") == "local" {
		if err := godotenv.Load(); err != nil {
			return nil, fmt.Errorf("load .env: %w", err)
		}
	}

	cfg := &Config{}
	if err := envconfig.Process("", &cfg.Server); err != nil {
		return nil, fmt.Errorf("process server config: %w", err)
	}
	if err := envconfig.Process("", &cfg.DB); err != nil {
		return nil, fmt.Errorf("process database config: %w", err)
	}

	if err := validator.New().Struct(cfg); err != nil {
		return nil, fmt.Errorf("validate config: %w", err)
	}

	return cfg, nil
}
