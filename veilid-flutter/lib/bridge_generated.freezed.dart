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
  AttachmentState get field0 => throw _privateConstructorUsedError;

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(AttachmentState field0) attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(AttachmentState field0)? attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(AttachmentState field0)? attachment,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Attachment value) attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Attachment value)? attachment,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Attachment value)? attachment,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;

  @JsonKey(ignore: true)
  $VeilidUpdateCopyWith<VeilidUpdate> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $VeilidUpdateCopyWith<$Res> {
  factory $VeilidUpdateCopyWith(
          VeilidUpdate value, $Res Function(VeilidUpdate) then) =
      _$VeilidUpdateCopyWithImpl<$Res>;
  $Res call({AttachmentState field0});
}

/// @nodoc
class _$VeilidUpdateCopyWithImpl<$Res> implements $VeilidUpdateCopyWith<$Res> {
  _$VeilidUpdateCopyWithImpl(this._value, this._then);

  final VeilidUpdate _value;
  // ignore: unused_field
  final $Res Function(VeilidUpdate) _then;

  @override
  $Res call({
    Object? field0 = freezed,
  }) {
    return _then(_value.copyWith(
      field0: field0 == freezed
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as AttachmentState,
    ));
  }
}

/// @nodoc
abstract class $AttachmentCopyWith<$Res>
    implements $VeilidUpdateCopyWith<$Res> {
  factory $AttachmentCopyWith(
          Attachment value, $Res Function(Attachment) then) =
      _$AttachmentCopyWithImpl<$Res>;
  @override
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
    required TResult Function(AttachmentState field0) attachment,
  }) {
    return attachment(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(AttachmentState field0)? attachment,
  }) {
    return attachment?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
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
    required TResult Function(Attachment value) attachment,
  }) {
    return attachment(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Attachment value)? attachment,
  }) {
    return attachment?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
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

  @override
  AttachmentState get field0;
  @override
  @JsonKey(ignore: true)
  $AttachmentCopyWith<Attachment> get copyWith =>
      throw _privateConstructorUsedError;
}
