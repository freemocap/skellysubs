// VideoSubtitleEditor.tsx
import {useEffect, useRef, useState} from "react"
import {
    Box,
    Button,
    Card,
    CardContent,
    CardHeader,
    Grid2,
    IconButton,
    Stack,
    Typography,
    styled, Grid
} from "@mui/material"
import {
    PlayArrow,
    Pause,
    VolumeUp,
    VolumeOff,
} from "@mui/icons-material"
import {Editor} from "@monaco-editor/react";
import {useAppSelector} from "../store/hooks";
import {selectProcessingContext} from "../store/slices/processing-status/processingStatusSlice";

// Define Subtitle type
interface Subtitle {
    start: number;
    end: number;
    text: string[];
}


// Styled components
const VideoWrapper = styled(Box)({
    position: 'relative',
    backgroundColor: 'black',
    aspectRatio: '16/9'
})

const VideoControls = styled(Box)({
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    background: 'linear-gradient(to top, rgba(0,0,0,0.8), transparent)',
    padding: '1rem'
})

// Sample VTT content
const sampleVTT = `WEBVTT

00:00:01.000 --> 00:00:04.000
Welcome to our demonstration video

00:00:05.000 --> 00:00:08.000
This video shows how to use the subtitle editor

00:00:10.000 --> 00:00:15.000
The current subtitle will be highlighted in the editor

00:00:16.000 --> 00:00:20.000
You can also edit the subtitles in real-time

00:00:22.000 --> 00:00:26.000
Thank you for watching our demonstration!`

// Parse VTT content into subtitle segments
function parseVTT(vttContent: string) {
    const lines = vttContent.split("\n")
    const subtitles: Subtitle[] = []
    let currentSubtitle: Subtitle | null = null

    for (const line of lines) {
        if (line.includes("-->")) {
            const [startTime, endTime] = line.split("-->").map((time) => {
                const [hours, minutes, seconds] = time.trim().split(/[:.]/).map(Number.parseFloat)
                return hours * 3600 + minutes * 60 + seconds + (seconds % 1)
            })

            currentSubtitle = {
                start: startTime,
                end: endTime,
                text: [],
            }
        } else if (line.trim() !== "" && line.trim() !== "WEBVTT" && currentSubtitle) {
            currentSubtitle.text.push(line.trim())

            if (!lines[lines.indexOf(line) + 1]?.includes("-->") && lines[lines.indexOf(line) + 1]?.trim() === "") {
                subtitles.push(currentSubtitle)
                currentSubtitle = null
            }
        }
    }

    if (currentSubtitle) {
        subtitles.push(currentSubtitle)
    }

    return subtitles
}

// Find the line range in the editor for a specific subtitle
function findSubtitleLineRange(vttContent: string, subtitle: Subtitle) {
    const lines = vttContent.split("\n")
    const startTimeStr = formatTime(subtitle.start)
    const endTimeStr = formatTime(subtitle.end)
    const timelineStr = `${startTimeStr} --> ${endTimeStr}`

    const timelineIndex = lines.findIndex((line) => line.includes(timelineStr))
    if (timelineIndex === -1) return null

    const startLine = timelineIndex + 1
    const endLine = startLine + subtitle.text.length - 1

    return {startLine, endLine}
}

// Format seconds to HH:MM:SS.mmm
function formatTime(seconds: number) {
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = Math.floor(seconds % 60)
    const millis = Math.floor((seconds % 1) * 1000)

    return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`
}

export default function VideoSubtitleEditor() {
    const videoRef = useRef<HTMLVideoElement>(null)
    const [isPlaying, setIsPlaying] = useState(false)
    const [isMuted, setIsMuted] = useState(false)
    const [currentTime, setCurrentTime] = useState(0)
    const [vttContent, setVttContent] = useState(sampleVTT)
    const [subtitles, setSubtitles] = useState<Subtitle[]>([])
    const [currentSubtitle, setCurrentSubtitle] = useState<Subtitle | null>(null)
    const [editorDecorations, setEditorDecorations] = useState<any[]>([])
    const editorRef = useRef<any>(null)
    const monacoRef = useRef<any>(null)
    const timeUpdateRef = useRef<boolean>(false)
    const {originalFile, mp3Audio} = useAppSelector(selectProcessingContext)
    const mediaFile = originalFile || mp3Audio

    useEffect(() => {
        const parsed = parseVTT(vttContent)
        setSubtitles(parsed)
    }, [vttContent])

    useEffect(() => {
        const handleTimeUpdate = () => {
            if (videoRef.current && !timeUpdateRef.current) {
                timeUpdateRef.current = true
                setCurrentTime(videoRef.current.currentTime)
                setTimeout(() => {
                    timeUpdateRef.current = false
                }, 50)
            }
        }

        const video = videoRef.current
        video?.addEventListener("timeupdate", handleTimeUpdate)

        return () => {
            video?.removeEventListener("timeupdate", handleTimeUpdate)
        }
    }, [])

    useEffect(() => {
        const current = subtitles.find((sub) => currentTime >= sub.start && currentTime <= sub.end)
        if (JSON.stringify(current) !== JSON.stringify(currentSubtitle)) {
            setCurrentSubtitle(current || null)
        }
    }, [currentTime, subtitles, currentSubtitle])

    useEffect(() => {
        if (editorRef.current && monacoRef.current && currentSubtitle) {
            const range = findSubtitleLineRange(vttContent, currentSubtitle)
            if (range) {
                const newDecorations = [{
                    range: new monacoRef.current.Range(range.startLine, 0, range.endLine + 1, 0),
                    options: {
                        isWholeLine: true,
                        className: "current-subtitle-highlight",
                    },
                }]
                const decorations = editorRef.current.deltaDecorations(editorDecorations, newDecorations)
                setEditorDecorations(decorations)
                editorRef.current.revealLineInCenter(range.startLine)
            }
        }
    }, [currentSubtitle, vttContent])

    const togglePlay = () => {
        if (videoRef.current) {
            isPlaying ? videoRef.current.pause() : videoRef.current.play()
            setIsPlaying(!isPlaying)
        }
    }

    const toggleMute = () => {
        if (videoRef.current) {
            videoRef.current.muted = !isMuted
            setIsMuted(!isMuted)
        }
    }

    const handleEditorDidMount = (editor: any, monaco: any) => {
        editorRef.current = editor
        monacoRef.current = monaco
    }

    const handleEditorChange = (value: string | undefined) => {
        value && setVttContent(value)
    }

    const handleTimelineClick = (subtitle: Subtitle) => {
        if (videoRef.current) {
            videoRef.current.currentTime = subtitle.start
            if (!isPlaying) {
                videoRef.current.play()
                setIsPlaying(true)
            }
        }
    }

    return (
        <Box
            sx={{
                p: 4,
                width: "100%",
                margin: "0 auto",
                border: "2px solid red",
                borderRadius: 2,
            }}
        >
            <Typography variant="h4" gutterBottom>
                Video Subtitle Editor
            </Typography>

            <Grid2 container spacing={3}>
                {/* Video Player */}
                <Grid item xs={12} md={6}>
                    <Card>
                        <VideoWrapper>
                            <video
                                ref={videoRef}
                                style={{
                                    width: '100%',
                                    height: mediaFile?.type?.startsWith('audio') ? 'auto' : '100%'
                                }}
                                src={mediaFile?.url ||
                                    "https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4"
                                }
                                onPlay={() => setIsPlaying(true)}
                                onPause={() => setIsPlaying(false)}
                            >
                                {mediaFile?.type?.startsWith('audio') && (
                                    <Typography variant="h6" sx={{color: 'white', textAlign: 'center'}}>
                                        Audio Visualization
                                    </Typography>
                                )}
                                    <track
                                    kind="subtitles"
                                    src={URL.createObjectURL(
                                    new Blob([vttContent], {type: "text/vtt"}),
                                    )}
                                srcLang="en"
                                label="English"
                                default
                            />
                        </video>

                        <VideoControls>
                            <Stack direction="row" alignItems="center" spacing={1}>
                                <IconButton
                                    onClick={togglePlay}
                                    sx={{
                                        color: "common.white",
                                        "&:hover": {bgcolor: "rgba(255,255,255,0.2)"},
                                    }}
                                >
                                    {isPlaying ? <Pause/> : <PlayArrow/>}
                                </IconButton>

                                <IconButton
                                    onClick={toggleMute}
                                    sx={{
                                        color: "common.white",
                                        "&:hover": {bgcolor: "rgba(255,255,255,0.2)"},
                                    }}
                                >
                                    {isMuted ? <VolumeOff/> : <VolumeUp/>}
                                </IconButton>

                                <Typography
                                    variant="body2"
                                    sx={{color: "common.white", ml: 1}}
                                >
                                    {Math.floor(currentTime / 60)}:
                                    {Math.floor(currentTime % 60)
                                        .toString()
                                        .padStart(2, "0")}
                                </Typography>

                                <Typography
                                    variant="body2"
                                    sx={{color: "rgba(255,255,255,0.6)"}}
                                >
                                    {currentSubtitle?.text.join(" ") || ""}
                                </Typography>
                            </Stack>
                        </VideoControls>
                    </VideoWrapper>

                    <CardContent sx={{bgcolor: "action.hover"}}>
                        <Typography variant="subtitle1" gutterBottom>
                            Current Subtitle
                        </Typography>
                        <Box sx={{minHeight: 80, transition: "all 300ms"}}>
                            {currentSubtitle ? (
                                <Box
                                    sx={{
                                        p: 2,
                                        bgcolor: "primary.light",
                                        border: "1px solid",
                                        borderColor: "primary.main",
                                    }}
                                >
                                    <Typography variant="caption" color="text.secondary">
                                        {formatTime(currentSubtitle.start)} →{" "}
                                        {formatTime(currentSubtitle.end)}
                                    </Typography>
                                    <Typography>{currentSubtitle.text.join(" ")}</Typography>
                                </Box>
                            ) : (
                                <Box sx={{p: 2, bgcolor: "background.default"}}>
                                    <Typography>No subtitle at current time</Typography>
                                </Box>
                            )}
                        </Box>
                    </CardContent>
                </Card>
            </Grid>

            {/* Subtitle Editor */}
            <Grid item xs={12} md={6}>
                <Card>
                    <CardHeader
                        title="Subtitle Editor"
                        subheader="Edit the WebVTT file directly. Changes apply in real-time."
                        sx={{borderBottom: "1px solid", borderColor: "divider"}}
                    />
                    <Box sx={{height: 400}}>
                        <Editor
                            height="100%"
                            language="plaintext"
                            theme="vs-dark"
                            value={vttContent}
                            onChange={handleEditorChange}
                            onMount={handleEditorDidMount}
                            options={{
                                minimap: {enabled: false},
                                lineNumbers: "on",
                                scrollBeyondLastLine: false,
                                wordWrap: "on",
                            }}
                        />
                    </Box>
                </Card>
            </Grid>

            {/* Subtitle Timeline */}
            <Grid item xs={12}>
                <Card>
                    <CardContent>
                        <Typography variant="subtitle1" gutterBottom>
                            Subtitle Timeline
                        </Typography>
                        <Stack direction="row" flexWrap="wrap" gap={1}>
                            {subtitles.map((subtitle, index) => (
                                <Button
                                    key={index}
                                    variant={
                                        currentSubtitle === subtitle ? "contained" : "outlined"
                                    }
                                    size="small"
                                    onClick={() => handleTimelineClick(subtitle)}
                                >
                                    {index + 1}
                                </Button>
                            ))}
                        </Stack>
                    </CardContent>
                </Card>
            </Grid>
        </Grid2>
</Box>
)
}