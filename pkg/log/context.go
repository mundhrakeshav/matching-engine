package log

import (
	"context"

	"go.uber.org/zap"
)

const (
	FieldRequestID  = "request_id"
	FieldTrackingID = "tracking_id"
)

type loggerCtxKey struct{}
type requestIDCtxKey struct{}
type trackingIDCtxKey struct{}

var defaultLogger = newDefaultLogger()

func SetLogger(ctx context.Context, logger Logger) context.Context {
	return context.WithValue(ctx, loggerCtxKey{}, logger)
}

func SetLoggerWithFields(ctx context.Context, logger Logger, reqID, trackingID string) context.Context {
	child := logger.With(
		String(FieldRequestID, reqID),
		String(FieldTrackingID, trackingID),
	)
	return SetLogger(ctx, child)
}

func GetLogger(ctx context.Context) Logger {
	if logger, ok := ctx.Value(loggerCtxKey{}).(Logger); ok {
		return logger
	}
	return defaultLogger
}

func WithRequestID(ctx context.Context, requestID string) context.Context {
	return context.WithValue(ctx, requestIDCtxKey{}, requestID)
}

func WithTrackingID(ctx context.Context, trackingID string) context.Context {
	return context.WithValue(ctx, trackingIDCtxKey{}, trackingID)
}

func RequestIDFromContext(ctx context.Context) string {
	id, _ := ctx.Value(requestIDCtxKey{}).(string)
	return id
}

func TrackingIDFromContext(ctx context.Context) string {
	id, _ := ctx.Value(trackingIDCtxKey{}).(string)
	return id
}

func requestIDFieldFromContext(ctx context.Context) (Field, bool) {
	id := RequestIDFromContext(ctx)
	if id == "" {
		return Field{}, false
	}
	return zap.String(FieldRequestID, id), true
}

func trackingIDFieldFromContext(ctx context.Context) (Field, bool) {
	id := TrackingIDFromContext(ctx)
	if id == "" {
		return Field{}, false
	}
	return zap.String(FieldTrackingID, id), true
}

func newDefaultLogger() Logger {
	cfg := zap.NewProductionConfig()
	cfg.OutputPaths = []string{"stdout"}
	cfg.ErrorOutputPaths = []string{"stderr"}

	zl, err := cfg.Build(zap.Fields(zap.String(fieldService, "default")))
	if err != nil {
		panic(err)
	}

	return &zapLogger{logger: zl}
}
