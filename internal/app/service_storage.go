package app

import (
	"ob/config"
	"ob/internal/matching"
)

type ServiceStorage struct {
	Engine *matching.Engine
}

func NewServiceStorage(cfg *config.Config, is *IntegrationStorage) (*ServiceStorage, error) {
	_ = cfg
	_ = is

	return &ServiceStorage{}, nil
}
