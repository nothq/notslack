#import <AudioToolbox/AudioToolbox.h>
#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <dispatch/dispatch.h>

#include <cstdint>
#include <mutex>

enum NotslackVideoCaptureState {
  NotslackVideoCaptureStateReady = 1,
  NotslackVideoCaptureStatePreviewing = 2,
  NotslackVideoCaptureStateRecording = 3,
  NotslackVideoCaptureStateDurationLimitReached = 4,
  NotslackVideoCaptureStateFailed = 5,
};

enum NotslackVideoCaptureErrorCode {
  NotslackVideoCaptureErrorCameraPermissionDenied = 1,
  NotslackVideoCaptureErrorMicrophonePermissionDenied = 2,
  NotslackVideoCaptureErrorNoCameraDevice = 3,
  NotslackVideoCaptureErrorNoMicrophoneDevice = 4,
  NotslackVideoCaptureErrorUnsupportedCameraFormat = 5,
  NotslackVideoCaptureErrorConfiguration = 6,
  NotslackVideoCaptureErrorRecording = 7,
  NotslackVideoCaptureErrorFinalization = 8,
};

typedef void (*notslack_video_capture_preview_cb)(void *context, CVPixelBufferRef pixel_buffer);
typedef void (*notslack_video_capture_error_cb)(void *context, int32_t code, const char *message);

struct NotslackVideoCaptureSession;

@interface NotslackVideoCaptureDelegate
    : NSObject <AVCaptureVideoDataOutputSampleBufferDelegate,
                AVCaptureFileOutputRecordingDelegate>
@property(nonatomic, assign) NotslackVideoCaptureSession *owner;
@end

struct NotslackVideoCaptureSession {
  __strong AVCaptureSession *captureSession;
  __strong AVCaptureMovieFileOutput *movieOutput;
  __strong AVCaptureVideoDataOutput *previewOutput;
  __strong NotslackVideoCaptureDelegate *delegate;
  __strong NSURL *scratchMovieURL;
  dispatch_queue_t previewQueue;
  dispatch_semaphore_t startedSemaphore;
  dispatch_semaphore_t finishedSemaphore;
  void *context;
  notslack_video_capture_preview_cb previewCallback;
  notslack_video_capture_error_cb errorCallback;
  std::mutex stateMutex;
  NotslackVideoCaptureState state;
  uint64_t durationMicros;
  bool started;
  bool recordingRequested;
  bool finished;
  bool stopRequested;
  bool resourcesStopped;
};

static constexpr int32_t kCaptureWidth = 1280;
static constexpr int32_t kCaptureHeight = 720;
static constexpr int32_t kCaptureFramesPerSecond = 30;
static constexpr double kCaptureMaximumSeconds = 300.0;
static constexpr int32_t kCaptureVideoBitRate = 4'000'000;
static constexpr int32_t kCaptureAudioBitRate = 128'000;

static uint64_t notslack_duration_micros(CMTime duration) {
  if (!CMTIME_IS_NUMERIC(duration)) {
    return 0;
  }
  CMTime scaled =
      CMTimeConvertScale(duration, 1'000'000, kCMTimeRoundingMethod_Default);
  return scaled.value > 0 ? static_cast<uint64_t>(scaled.value) : 0;
}

static void notslack_emit_error(notslack_video_capture_error_cb callback, void *context,
                            NotslackVideoCaptureErrorCode code, NSString *message) {
  if (callback == nullptr) {
    return;
  }
  NSString *diagnostic = message ?: @"AVFoundation video capture failed";
  callback(context, static_cast<int32_t>(code), diagnostic.UTF8String);
}

static void notslack_fail_session(NotslackVideoCaptureSession *session,
                              NotslackVideoCaptureErrorCode code, NSString *message) {
  {
    std::lock_guard<std::mutex> lock(session->stateMutex);
    session->state = NotslackVideoCaptureStateFailed;
  }
  notslack_emit_error(session->errorCallback, session->context, code, message);
}

static bool notslack_request_access(AVMediaType mediaType,
                                NotslackVideoCaptureErrorCode denialCode,
                                notslack_video_capture_error_cb errorCallback,
                                void *context) {
  AVAuthorizationStatus status =
      [AVCaptureDevice authorizationStatusForMediaType:mediaType];
  if (status == AVAuthorizationStatusAuthorized) {
    return true;
  }
  if (status == AVAuthorizationStatusDenied ||
      status == AVAuthorizationStatusRestricted) {
    notslack_emit_error(errorCallback, context, denialCode, @"capture access was denied");
    return false;
  }

  dispatch_semaphore_t permissionSemaphore = dispatch_semaphore_create(0);
  __block BOOL granted = NO;
  [AVCaptureDevice requestAccessForMediaType:mediaType
                           completionHandler:^(BOOL accessGranted) {
                             granted = accessGranted;
                             dispatch_semaphore_signal(permissionSemaphore);
                           }];
  // AVFoundation invokes the access completion handler after the user resolves
  // the system prompt. A deadline would incorrectly end RequestingPermissions
  // while the prompt is still legitimately pending.
  dispatch_semaphore_wait(permissionSemaphore, DISPATCH_TIME_FOREVER);
  if (!granted) {
    notslack_emit_error(errorCallback, context, denialCode, @"capture access was denied");
  }
  return granted;
}

static AVCaptureDeviceFormat *notslack_camera_format(AVCaptureDevice *camera) {
  for (AVCaptureDeviceFormat *format in camera.formats) {
    CMVideoDimensions dimensions =
        CMVideoFormatDescriptionGetDimensions(format.formatDescription);
    if (dimensions.width != kCaptureWidth || dimensions.height != kCaptureHeight) {
      continue;
    }
    for (AVFrameRateRange *range in format.videoSupportedFrameRateRanges) {
      if (range.minFrameRate <= kCaptureFramesPerSecond &&
          range.maxFrameRate >= kCaptureFramesPerSecond) {
        return format;
      }
    }
  }
  return nil;
}

static bool notslack_configure_camera(AVCaptureDevice *camera,
                                  notslack_video_capture_error_cb errorCallback,
                                  void *context) {
  AVCaptureDeviceFormat *format = notslack_camera_format(camera);
  if (format == nil) {
    notslack_emit_error(errorCallback, context,
                    NotslackVideoCaptureErrorUnsupportedCameraFormat,
                    @"the camera has no 1280x720 format supporting 30 fps");
    return false;
  }
  NSError *configurationError = nil;
  if (![camera lockForConfiguration:&configurationError]) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    configurationError.localizedDescription);
    return false;
  }
  @try {
    camera.activeFormat = format;
    camera.activeVideoMinFrameDuration =
        CMTimeMake(1, kCaptureFramesPerSecond);
    camera.activeVideoMaxFrameDuration =
        CMTimeMake(1, kCaptureFramesPerSecond);
  } @catch (NSException *exception) {
    [camera unlockForConfiguration];
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    exception.reason);
    return false;
  }
  [camera unlockForConfiguration];
  return true;
}

static bool notslack_add_input(AVCaptureSession *session, AVCaptureDevice *device,
                           notslack_video_capture_error_cb errorCallback, void *context) {
  NSError *inputError = nil;
  AVCaptureDeviceInput *input =
      [AVCaptureDeviceInput deviceInputWithDevice:device error:&inputError];
  if (input == nil) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    inputError.localizedDescription);
    return false;
  }
  if (![session canAddInput:input]) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"AVCaptureSession rejected a required capture input");
    return false;
  }
  [session addInput:input];
  return true;
}

static bool notslack_configure_movie_output(
    AVCaptureSession *session, AVCaptureMovieFileOutput *movieOutput,
    notslack_video_capture_error_cb errorCallback, void *context) {
  if (![session canAddOutput:movieOutput]) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"AVCaptureSession rejected the movie output");
    return false;
  }
  [session addOutput:movieOutput];

  AVCaptureConnection *videoConnection =
      [movieOutput connectionWithMediaType:AVMediaTypeVideo];
  AVCaptureConnection *audioConnection =
      [movieOutput connectionWithMediaType:AVMediaTypeAudio];
  if (videoConnection == nil || audioConnection == nil) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"the movie output is missing a video or audio connection");
    return false;
  }
  NSDictionary *videoSettings = @{
    AVVideoCodecKey : AVVideoCodecTypeH264,
    AVVideoCompressionPropertiesKey : @{
      AVVideoAverageBitRateKey : @(kCaptureVideoBitRate),
      AVVideoExpectedSourceFrameRateKey : @(kCaptureFramesPerSecond),
    },
  };
  NSDictionary *audioSettings = @{
    AVFormatIDKey : @(kAudioFormatMPEG4AAC),
    AVEncoderBitRateKey : @(kCaptureAudioBitRate),
  };
  NSDictionary *appliedVideoSettings = nil;
  NSDictionary *appliedAudioSettings = nil;
  @try {
    if (videoConnection.isVideoMirroringSupported) {
      videoConnection.videoMirrored = NO;
    }
    [movieOutput setOutputSettings:videoSettings forConnection:videoConnection];
    [movieOutput setOutputSettings:audioSettings forConnection:audioConnection];
    appliedVideoSettings =
        [movieOutput outputSettingsForConnection:videoConnection];
    appliedAudioSettings =
        [movieOutput outputSettingsForConnection:audioConnection];
    movieOutput.maxRecordedDuration =
        CMTimeMakeWithSeconds(kCaptureMaximumSeconds, 600);
  } @catch (NSException *exception) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    exception.reason);
    return false;
  }
  if (![appliedVideoSettings[AVVideoCodecKey] isEqual:AVVideoCodecTypeH264] ||
      [appliedAudioSettings[AVFormatIDKey] unsignedIntValue] !=
          kAudioFormatMPEG4AAC) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"AVFoundation did not accept H.264 video and AAC audio");
    return false;
  }
  return true;
}

static bool notslack_configure_preview_output(
    AVCaptureSession *session, AVCaptureVideoDataOutput *previewOutput,
    NotslackVideoCaptureDelegate *delegate, dispatch_queue_t previewQueue,
    notslack_video_capture_error_cb errorCallback, void *context) {
  @try {
    previewOutput.alwaysDiscardsLateVideoFrames = YES;
    previewOutput.videoSettings = @{
      (id)kCVPixelBufferPixelFormatTypeKey :
          @(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange),
      (id)kCVPixelBufferIOSurfacePropertiesKey : @{},
      (id)kCVPixelBufferMetalCompatibilityKey : @YES,
    };
    [previewOutput setSampleBufferDelegate:delegate queue:previewQueue];
  } @catch (NSException *exception) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    exception.reason);
    return false;
  }
  if (![session canAddOutput:previewOutput]) {
    [previewOutput setSampleBufferDelegate:nil queue:nullptr];
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"AVCaptureSession rejected the preview output");
    return false;
  }
  [session addOutput:previewOutput];
  AVCaptureConnection *previewConnection =
      [previewOutput connectionWithMediaType:AVMediaTypeVideo];
  if (previewConnection == nil ||
      !previewConnection.isVideoMirroringSupported) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    @"the camera preview connection does not support mirroring");
    return false;
  }
  @try {
    previewConnection.videoMirrored = YES;
  } @catch (NSException *exception) {
    notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                    exception.reason);
    return false;
  }
  return true;
}

static bool notslack_detach_and_drain_preview(NotslackVideoCaptureSession *session) {
  @try {
    [session->previewOutput setSampleBufferDelegate:nil queue:nullptr];
    if (session->previewQueue != nullptr) {
      dispatch_sync(session->previewQueue, ^{});
    }
    return true;
  } @catch (NSException *exception) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      exception.reason);
    return false;
  }
}

@implementation NotslackVideoCaptureDelegate

- (void)captureOutput:(AVCaptureOutput *)output
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
       fromConnection:(AVCaptureConnection *)connection {
  (void)output;
  (void)connection;
  NotslackVideoCaptureSession *owner = self.owner;
  if (owner == nullptr || owner->previewCallback == nullptr) {
    return;
  }
  {
    std::lock_guard<std::mutex> lock(owner->stateMutex);
    if (owner->state == NotslackVideoCaptureStateReady) {
      owner->state = NotslackVideoCaptureStatePreviewing;
    }
  }
  CVPixelBufferRef pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
  if (pixelBuffer == nullptr) {
    return;
  }
  CFRetain(pixelBuffer);
  owner->previewCallback(owner->context, pixelBuffer);
}

- (void)captureOutput:(AVCaptureFileOutput *)output
    didStartRecordingToOutputFileAtURL:(NSURL *)fileURL
    fromConnections:(NSArray<AVCaptureConnection *> *)connections {
  (void)output;
  (void)fileURL;
  (void)connections;
  NotslackVideoCaptureSession *owner = self.owner;
  if (owner == nullptr) {
    return;
  }
  {
    std::lock_guard<std::mutex> lock(owner->stateMutex);
    owner->started = true;
    if (owner->state != NotslackVideoCaptureStateFailed) {
      owner->state = NotslackVideoCaptureStateRecording;
    }
  }
  dispatch_semaphore_signal(owner->startedSemaphore);
}

- (void)captureOutput:(AVCaptureFileOutput *)output
    didFinishRecordingToOutputFileAtURL:(NSURL *)outputFileURL
    fromConnections:(NSArray<AVCaptureConnection *> *)connections
    error:(NSError *)error {
  (void)outputFileURL;
  (void)connections;
  NotslackVideoCaptureSession *owner = self.owner;
  if (owner == nullptr) {
    return;
  }
  uint64_t durationMicros =
      notslack_duration_micros(((AVCaptureMovieFileOutput *)output).recordedDuration);
  bool successful = error == nil;
  if (error != nil) {
    NSNumber *finished =
        error.userInfo[AVErrorRecordingSuccessfullyFinishedKey];
    successful = finished.boolValue;
  }

  bool unexpectedFinish = false;
  bool started = false;
  {
    std::lock_guard<std::mutex> lock(owner->stateMutex);
    owner->durationMicros = durationMicros;
    owner->finished = true;
    started = owner->started;
    if (successful &&
        durationMicros >=
            static_cast<uint64_t>((kCaptureMaximumSeconds - 0.1) * 1'000'000)) {
      owner->state = NotslackVideoCaptureStateDurationLimitReached;
    } else if (!successful) {
      owner->state = NotslackVideoCaptureStateFailed;
    } else if (!owner->stopRequested) {
      owner->state = NotslackVideoCaptureStateFailed;
      unexpectedFinish = true;
    }
  }
  if (!successful || unexpectedFinish) {
    NSString *message = unexpectedFinish
                            ? @"AVFoundation ended the recording unexpectedly"
                            : error.localizedDescription;
    notslack_emit_error(owner->errorCallback, owner->context,
                    NotslackVideoCaptureErrorFinalization, message);
  }
  if (!started) {
    dispatch_semaphore_signal(owner->startedSemaphore);
  }
  dispatch_semaphore_signal(owner->finishedSemaphore);
}

@end

static bool notslack_stop_capture_resources(NotslackVideoCaptureSession *session) {
  bool finished = false;
  bool started = false;
  {
    std::lock_guard<std::mutex> lock(session->stateMutex);
    if (session->resourcesStopped) {
      return true;
    }
    session->stopRequested = true;
    finished = session->finished;
    started = session->started;
  }

  bool stoppedMovie = true;
  @try {
    if (started && !finished) {
      if (session->movieOutput.isRecording) {
        [session->movieOutput stopRecording];
      }
      // AVCaptureFileOutput guarantees didFinish for every recording request,
      // including start failures. The delegate owns the session until it
      // signals this semaphore, so timing out and freeing it would be unsafe.
      dispatch_semaphore_wait(session->finishedSemaphore,
                              DISPATCH_TIME_FOREVER);
    }
  } @catch (NSException *exception) {
    stoppedMovie = false;
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      exception.reason);
  }

  bool stoppedSession = true;
  @try {
    [session->captureSession stopRunning];
  } @catch (NSException *exception) {
    stoppedSession = false;
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      exception.reason);
  }

  bool drainedPreview = notslack_detach_and_drain_preview(session);
  bool stopped = stoppedMovie && stoppedSession && drainedPreview;
  if (stopped) {
    std::lock_guard<std::mutex> lock(session->stateMutex);
    session->resourcesStopped = true;
  }
  return stopped;
}

static bool notslack_destroy_session_internal(NotslackVideoCaptureSession *session) {
  if (!notslack_stop_capture_resources(session)) {
    return false;
  }
  session->delegate.owner = nullptr;
  delete session;
  return true;
}

extern "C" NotslackVideoCaptureSession *notslack_video_capture_session_prepare(
    void *context,
    notslack_video_capture_preview_cb previewCallback,
    notslack_video_capture_error_cb errorCallback) {
  @autoreleasepool {
    NotslackVideoCaptureSession *session = nullptr;
    @try {
    if (!notslack_request_access(AVMediaTypeVideo,
                             NotslackVideoCaptureErrorCameraPermissionDenied,
                             errorCallback, context) ||
        !notslack_request_access(AVMediaTypeAudio,
                             NotslackVideoCaptureErrorMicrophonePermissionDenied,
                             errorCallback, context)) {
      return nullptr;
    }

    AVCaptureDevice *camera =
        [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
    AVCaptureDevice *microphone =
        [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeAudio];
    if (camera == nil) {
      notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorNoCameraDevice,
                      @"no default camera device is available");
      return nullptr;
    }
    if (microphone == nil) {
      notslack_emit_error(errorCallback, context,
                      NotslackVideoCaptureErrorNoMicrophoneDevice,
                      @"no default microphone device is available");
      return nullptr;
    }
    AVCaptureSession *captureSession = [AVCaptureSession new];
    if (![captureSession canSetSessionPreset:AVCaptureSessionPreset1280x720]) {
      notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorConfiguration,
                      @"AVCaptureSession does not support the 1280x720 preset");
      return nullptr;
    }
    @try {
      captureSession.sessionPreset = AVCaptureSessionPreset1280x720;
    } @catch (NSException *exception) {
      notslack_emit_error(errorCallback, context,
                      NotslackVideoCaptureErrorConfiguration, exception.reason);
      return nullptr;
    }
    if (!notslack_configure_camera(camera, errorCallback, context)) {
      return nullptr;
    }
    [captureSession beginConfiguration];
    bool inputsConfigured =
        notslack_add_input(captureSession, camera, errorCallback, context) &&
        notslack_add_input(captureSession, microphone, errorCallback, context);
    if (!inputsConfigured) {
      [captureSession commitConfiguration];
      return nullptr;
    }

    session = new NotslackVideoCaptureSession();
    session->context = context;
    session->previewCallback = previewCallback;
    session->errorCallback = errorCallback;
    session->captureSession = captureSession;
    session->movieOutput = [AVCaptureMovieFileOutput new];
    session->previewOutput = [AVCaptureVideoDataOutput new];
    session->delegate = [NotslackVideoCaptureDelegate new];
    session->delegate.owner = session;
    session->previewQueue =
        dispatch_queue_create("dev.nothq.notslack.video.capture.preview",
                              DISPATCH_QUEUE_SERIAL);
    session->startedSemaphore = dispatch_semaphore_create(0);
    session->finishedSemaphore = dispatch_semaphore_create(0);
    session->state = NotslackVideoCaptureStateReady;
    session->durationMicros = 0;
    session->started = false;
    session->recordingRequested = false;
    session->finished = false;
    session->stopRequested = false;
    session->resourcesStopped = false;

    bool outputsConfigured =
        notslack_configure_movie_output(captureSession, session->movieOutput,
                                    errorCallback, context) &&
        notslack_configure_preview_output(
            captureSession, session->previewOutput, session->delegate,
            session->previewQueue, errorCallback, context);
    [captureSession commitConfiguration];
    if (!outputsConfigured) {
      return notslack_destroy_session_internal(session) ? nullptr : session;
    }

    [captureSession startRunning];
    if (!captureSession.isRunning) {
      notslack_emit_error(errorCallback, context, NotslackVideoCaptureErrorRecording,
                      @"AVCaptureSession did not start");
      return notslack_destroy_session_internal(session) ? nullptr : session;
    }
    return session;
    } @catch (NSException *exception) {
      if (session == nullptr) {
        notslack_emit_error(errorCallback, context,
                        NotslackVideoCaptureErrorConfiguration, exception.reason);
        return nullptr;
      }
      notslack_fail_session(session, NotslackVideoCaptureErrorConfiguration,
                        exception.reason);
      return notslack_destroy_session_internal(session) ? nullptr : session;
    }
  }
}

extern "C" bool notslack_video_capture_session_begin_recording(
    NotslackVideoCaptureSession *session, const char *scratchMoviePath) {
  if (session == nullptr || scratchMoviePath == nullptr ||
      scratchMoviePath[0] == '\0') {
    return false;
  }
  @autoreleasepool {
    @try {
      NSString *path = [NSString stringWithUTF8String:scratchMoviePath];
      if (path == nil ||
          [[NSFileManager defaultManager] fileExistsAtPath:path]) {
        notslack_fail_session(
            session, NotslackVideoCaptureErrorConfiguration,
            @"the scratch movie path is invalid or already exists");
        return false;
      }
      {
        std::lock_guard<std::mutex> lock(session->stateMutex);
        bool ready = session->state == NotslackVideoCaptureStatePreviewing;
        if (!ready || session->recordingRequested) {
          return false;
        }
        session->recordingRequested = true;
        session->scratchMovieURL = [NSURL fileURLWithPath:path];
      }
      [session->movieOutput
          startRecordingToOutputFileURL:session->scratchMovieURL
                      recordingDelegate:session->delegate];
      // didStart is optional on failure, but didFinish is guaranteed for every
      // recording request and signals the same semaphore when didStart did not.
      dispatch_semaphore_wait(session->startedSemaphore,
                              DISPATCH_TIME_FOREVER);
      std::lock_guard<std::mutex> lock(session->stateMutex);
      return session->started &&
             session->state == NotslackVideoCaptureStateRecording;
    } @catch (NSException *exception) {
      notslack_fail_session(session, NotslackVideoCaptureErrorRecording,
                        exception.reason);
      return false;
    }
  }
}

extern "C" int32_t notslack_video_capture_session_status(
    NotslackVideoCaptureSession *session, uint64_t *durationMicros) {
  if (session == nullptr) {
    return NotslackVideoCaptureStateFailed;
  }
  @autoreleasepool {
    @try {
      uint64_t liveDuration =
          notslack_duration_micros(session->movieOutput.recordedDuration);
      std::lock_guard<std::mutex> lock(session->stateMutex);
      session->durationMicros =
          liveDuration > session->durationMicros ? liveDuration
                                                : session->durationMicros;
      if (durationMicros != nullptr) {
        *durationMicros = session->durationMicros;
      }
      return session->state;
    } @catch (NSException *exception) {
      notslack_fail_session(session, NotslackVideoCaptureErrorRecording,
                        exception.reason);
      if (durationMicros != nullptr) {
        std::lock_guard<std::mutex> lock(session->stateMutex);
        *durationMicros = session->durationMicros;
      }
      return NotslackVideoCaptureStateFailed;
    }
  }
}

static bool notslack_export_mp4(NotslackVideoCaptureSession *session,
                            const char *mp4OutputPath) {
  if (mp4OutputPath == nullptr || mp4OutputPath[0] == '\0' ||
      session->scratchMovieURL == nil) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      @"the MP4 finalization paths are unavailable");
    return false;
  }
  NSString *path = [NSString stringWithUTF8String:mp4OutputPath];
  if (path == nil || [[NSFileManager defaultManager] fileExistsAtPath:path]) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      @"the MP4 output path is invalid or already exists");
    return false;
  }
  AVURLAsset *asset =
      [AVURLAsset URLAssetWithURL:session->scratchMovieURL options:nil];
  AVAssetExportSession *exportSession =
      [[AVAssetExportSession alloc] initWithAsset:asset
                                      presetName:AVAssetExportPresetPassthrough];
  if (exportSession == nil ||
      ![exportSession.supportedFileTypes containsObject:AVFileTypeMPEG4]) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      @"AVFoundation cannot passthrough the capture as MP4");
    return false;
  }
  dispatch_semaphore_t exportSemaphore = dispatch_semaphore_create(0);
  @try {
    exportSession.outputURL = [NSURL fileURLWithPath:path];
    exportSession.outputFileType = AVFileTypeMPEG4;
    exportSession.shouldOptimizeForNetworkUse = YES;
    [exportSession exportAsynchronouslyWithCompletionHandler:^{
      dispatch_semaphore_signal(exportSemaphore);
    }];
    // AVAssetExportSession guarantees its completion handler for success,
    // failure, and cancellation, so the scratch lease remains valid until the
    // terminal export state is observable.
    dispatch_semaphore_wait(exportSemaphore, DISPATCH_TIME_FOREVER);
  } @catch (NSException *exception) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      exception.reason);
    return false;
  }
  if (exportSession.status != AVAssetExportSessionStatusCompleted) {
    notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                      exportSession.error.localizedDescription);
    return false;
  }
  return true;
}

static bool notslack_finish_recording(NotslackVideoCaptureSession *session,
                                  const char *mp4OutputPath,
                                  uint64_t *durationMicros) {
  if (session == nullptr) {
    return false;
  }
  @autoreleasepool {
    {
      std::lock_guard<std::mutex> lock(session->stateMutex);
      if (!session->started) {
        return false;
      }
    }
    bool stopped = notslack_stop_capture_resources(session);
    bool failed = false;
    {
      std::lock_guard<std::mutex> lock(session->stateMutex);
      if (durationMicros != nullptr) {
        *durationMicros = session->durationMicros;
      }
      failed = session->state == NotslackVideoCaptureStateFailed;
    }
    return stopped && !failed && notslack_export_mp4(session, mp4OutputPath);
  }
}

extern "C" bool notslack_video_capture_session_stop(
    NotslackVideoCaptureSession *session, const char *mp4OutputPath,
    uint64_t *durationMicros) {
  @try {
    return notslack_finish_recording(session, mp4OutputPath, durationMicros);
  } @catch (NSException *exception) {
    if (session != nullptr) {
      notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                        exception.reason);
    }
    return false;
  }
}

extern "C" bool
notslack_video_capture_session_cancel(NotslackVideoCaptureSession *session) {
  if (session == nullptr) {
    return true;
  }
  @try {
    bool stopped = notslack_stop_capture_resources(session);
    std::lock_guard<std::mutex> lock(session->stateMutex);
    return stopped && session->state != NotslackVideoCaptureStateFailed;
  } @catch (NSException *exception) {
    if (session != nullptr) {
      notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                        exception.reason);
    }
    return false;
  }
}

extern "C" bool
notslack_video_capture_session_destroy(NotslackVideoCaptureSession *session) {
  if (session == nullptr) {
    return true;
  }
  @autoreleasepool {
    @try {
      return notslack_destroy_session_internal(session);
    } @catch (NSException *exception) {
      notslack_fail_session(session, NotslackVideoCaptureErrorFinalization,
                        exception.reason);
      return false;
    }
  }
}
