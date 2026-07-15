package controller

import (
	v1 "ob/internal/controller/v1"
	"ob/internal/matching"
	"ob/pkg/httpserver"
)

func SetupRouter(
	serviceName string,
	router httpserver.Router,
	engine *matching.Engine,
	healthChecks ...CheckFunc,
) {
	registerHealthHandler(router)
	registerReadinessStatusHandler(router, serviceName)
	registerReadinessProbeHandler(router, healthChecks...)

	handler := v1.NewHandler(engine)

	apiV1 := router.Group("/v1")
	matchingV1 := apiV1.Group("/matching")
	{
		matchingV1.GET("/status", handler.MakeStatusHandler())
	}
}
