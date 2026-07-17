package app

import (
	"ob/config"
)

type IntegrationStorage struct {
	// DB, cache, and message broker clients will be wired here.
}

func NewIntegrationStorage(cfg *config.Config) (*IntegrationStorage, error) {
	_ = cfg

	return &IntegrationStorage{}, nil
}
