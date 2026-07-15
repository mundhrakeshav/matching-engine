package log

import (
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

// Field is a structured log field.
type Field = zapcore.Field

func String(key, value string) Field {
	return zap.String(key, value)
}

func Int(key string, value int) Field {
	return zap.Int(key, value)
}

func Err(err error) Field {
	return zap.Error(err)
}
