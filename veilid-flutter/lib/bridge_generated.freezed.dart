// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target

part of 'bridge_generated.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more informations: https://github.com/rrousselGit/freezed#custom-getters-and-methods');

/// @nodoc
class _$VeilidUpdateTearOff {
  const _$VeilidUpdateTearOff();

  Log log({required VeilidLogLevel logLevel, required String message}) {
    return Log(
      logLevel: logLevel,
      message: message,
    );
  }

  Attachment attachment(AttachmentState field0) {
    return Attachment(
      field0,
    );
  }
}

/// @nodoc
const $VeilidUpdate = _$VeilidUpdateTearOff();

/// @nodoc
mixin _$VeilidUpdate {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(VeilidLogLevel logLevel, String message) log,
    required TResult Function(AttachmentState field0) attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Log value) log,
    required TResult Function(Attachment value) attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $VeilidUpdateCopyWith<$Res> {
  factory $VeilidUpdateCopyWith(
          VeilidUpdate value, $Res Function(VeilidUpdate) then) =
      _$VeilidUpdateCopyWithImpl<$Res>;
}

/// @nodoc
class _$VeilidUpdateCopyWithImpl<$Res> implements $VeilidUpdateCopyWith<$Res> {
  _$VeilidUpdateCopyWithImpl(this._value, this._then);

  final VeilidUpdate _value;
  // ignore: unused_field
  final $Res Function(VeilidUpdate) _then;
}

/// @nodoc
abstract class $LogCopyWith<$Res> {
  factory $LogCopyWith(Log value, $Res Function(Log) then) =
      _$LogCopyWithImpl<$Res>;
  $Res call({VeilidLogLevel logLevel, String message});
}

/// @nodoc
class _$LogCopyWithImpl<$Res> extends _$VeilidUpdateCopyWithImpl<$Res>
    implements $LogCopyWith<$Res> {
  _$LogCopyWithImpl(Log _value, $Res Function(Log) _then)
      : super(_value, (v) => _then(v as Log));

  @override
  Log get _value => super._value as Log;

  @override
  $Res call({
    Object? logLevel = freezed,
    Object? message = freezed,
  }) {
    return _then(Log(
      logLevel: logLevel == freezed
          ? _value.logLevel
          : logLevel // ignore: cast_nullable_to_non_nullable
              as VeilidLogLevel,
      message: message == freezed
          ? _value.message
          : message // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class _$Log implements Log {
  const _$Log({required this.logLevel, required this.message});

  @override
  final VeilidLogLevel logLevel;
  @override
  final String message;

  @override
  String toString() {
    return 'VeilidUpdate.log(logLevel: $logLevel, message: $message)';
  }

  @override
  bool operator ==(dynamic other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Log &&
            const DeepCollectionEquality().equals(other.logLevel, logLevel) &&
            const DeepCollectionEquality().equals(other.message, message));
  }

  @override
  int get hashCode => Object.hash(
      runtimeType,
      const DeepCollectionEquality().hash(logLevel),
      const DeepCollectionEquality().hash(message));

  @JsonKey(ignore: true)
  @override
  $LogCopyWith<Log> get copyWith => _$LogCopyWithImpl<Log>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(VeilidLogLevel logLevel, String message) log,
    required TResult Function(AttachmentState field0) attachment,
  }) {
    return log(logLevel, message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
  }) {
    return log?.call(logLevel, message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
    required TResult orElse(),
  }) {
    if (log != null) {
      return log(logLevel, message);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Log value) log,
    required TResult Function(Attachment value) attachment,
  }) {
    return log(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
  }) {
    return log?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
    required TResult orElse(),
  }) {
    if (log != null) {
      return log(this);
    }
    return orElse();
  }
}

abstract class Log implements VeilidUpdate {
  const factory Log(
      {required VeilidLogLevel logLevel, required String message}) = _$Log;

  VeilidLogLevel get logLevel;
  String get message;
  @JsonKey(ignore: true)
  $LogCopyWith<Log> get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $AttachmentCopyWith<$Res> {
  factory $AttachmentCopyWith(
          Attachment value, $Res Function(Attachment) then) =
      _$AttachmentCopyWithImpl<$Res>;
  $Res call({AttachmentState field0});
}

/// @nodoc
class _$AttachmentCopyWithImpl<$Res> extends _$VeilidUpdateCopyWithImpl<$Res>
    implements $AttachmentCopyWith<$Res> {
  _$AttachmentCopyWithImpl(Attachment _value, $Res Function(Attachment) _then)
      : super(_value, (v) => _then(v as Attachment));

  @override
  Attachment get _value => super._value as Attachment;

  @override
  $Res call({
    Object? field0 = freezed,
  }) {
    return _then(Attachment(
      field0 == freezed
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as AttachmentState,
    ));
  }
}

/// @nodoc

class _$Attachment implements Attachment {
  const _$Attachment(this.field0);

  @override
  final AttachmentState field0;

  @override
  String toString() {
    return 'VeilidUpdate.attachment(field0: $field0)';
  }

  @override
  bool operator ==(dynamic other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is Attachment &&
            const DeepCollectionEquality().equals(other.field0, field0));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(field0));

  @JsonKey(ignore: true)
  @override
  $AttachmentCopyWith<Attachment> get copyWith =>
      _$AttachmentCopyWithImpl<Attachment>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(VeilidLogLevel logLevel, String message) log,
    required TResult Function(AttachmentState field0) attachment,
  }) {
    return attachment(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
  }) {
    return attachment?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(VeilidLogLevel logLevel, String message)? log,
    TResult Function(AttachmentState field0)? attachment,
    required TResult orElse(),
  }) {
    if (attachment != null) {
      return attachment(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Log value) log,
    required TResult Function(Attachment value) attachment,
  }) {
    return attachment(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
  }) {
    return attachment?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Log value)? log,
    TResult Function(Attachment value)? attachment,
    required TResult orElse(),
  }) {
    if (attachment != null) {
      return attachment(this);
    }
    return orElse();
  }
}

abstract class Attachment implements VeilidUpdate {
  const factory Attachment(AttachmentState field0) = _$Attachment;

  AttachmentState get field0;
  @JsonKey(ignore: true)
  $AttachmentCopyWith<Attachment> get copyWith =>
      throw _privateConstructorUsedError;
}
