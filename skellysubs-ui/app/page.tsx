import VideoAudioRecorder from "../components/VideoAudioRecorder"

export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-4">
      <h1 className="text-4xl font-bold mb-8">Video and Audio Recorder with Notes</h1>
      <VideoAudioRecorder />
    </main>
  )
}

