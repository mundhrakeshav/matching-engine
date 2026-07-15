package config

import (
	"fmt"
	"os"

	"github.com/go-playground/validator/v10"
	"github.com/joho/godotenv"
	"github.com/kelseyhightower/envconfig"
)

type Config struct {
	Port        int    `envconfig:"PORT" default:"8080" validate:"required,min=1,max=65535"`
	Host        string `envconfig:"HOST" validate:"required"`
	ServiceName string `envconfig:"SERVICE_NAME" validate:"required"`
	LogLevel    string `envconfig:"LOG_LEVEL" default:"info" validate:"required,oneof=debug info warn error"`
}

func Load() (*Config, error) {
	if os.Getenv("ENV") == "local" {
		if err := godotenv.Load(); err != nil {
			return nil, fmt.Errorf("load .env: %w", err)
		}
	}

	var cfg Config
	if err := envconfig.Process("", &cfg); err != nil {
		return nil, fmt.Errorf("process env config: %w", err)
	}

	if err := validator.New().Struct(&cfg); err != nil {
		return nil, fmt.Errorf("validate config: %w", err)
	}

	return &cfg, nil
}
