import 'dart:async';

import 'package:logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:veilid/veilid.dart';
import 'package:logger_flutter_viewer/logger_flutter_viewer.dart';

// Logger
var stacklog = Logger(
    printer: PrettyPrinter(
        methodCount: 10,
        errorMethodCount: 10,
        printTime: true,
        colors: true,
        printEmojis: true),
    output: ScreenOutput());
var log = Logger(
    printer: PrettyPrinter(
      methodCount: 0,
      errorMethodCount: 1,
      printTime: true,
      colors: true,
      printEmojis: true,
      noBoxingByDefault: true,
    ),
    output: ScreenOutput());
var barelog = Logger(
    printer: PrettyPrinter(
      methodCount: 0,
      errorMethodCount: 0,
      printTime: false,
      colors: true,
      printEmojis: true,
      noBoxingByDefault: true,
    ),
    output: ScreenOutput());

class ScreenOutput extends LogOutput {
  @override
  void output(OutputEvent event) {
    LogConsole.output(event);
  }
}

// Entrypoint
void main() {
  runApp(const MyApp());
}

// Main App
class MyApp extends StatefulWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
  String _veilidVersion = 'Unknown';

  @override
  void initState() {
    super.initState();
    initPlatformState();
  }

  // Platform messages are asynchronous, so we initialize in an async method.
  Future<void> initPlatformState() async {
    String veilidVersion;
    // Platform messages may fail, so we use a try/catch PlatformException.
    // We also handle the message potentially returning null.
    try {
      veilidVersion = Veilid.instance.veilidVersionString();
    } on PlatformException {
      veilidVersion = 'Failed to get veilid version.';
    }
    log.e("Error test");
    log.w("Warning test");
    stacklog.i("Info test with stacklog");
    barelog.d("debug bare-log test");

    // If the widget was removed from the tree while the asynchronous platform
    // message was in flight, we want to discard the reply rather than calling
    // setState to update our non-existent appearance.
    if (!mounted) return;

    setState(() {
      _veilidVersion = veilidVersion;
    });
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(
          title: Text('Veilid Plugin Version $_veilidVersion'),
        ),
        body: LogConsole(dark: Theme.of(context).brightness == Brightness.dark),
      ),
    );
  }
}
