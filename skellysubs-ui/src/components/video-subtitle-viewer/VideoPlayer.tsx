import { useRef, useState, useEffect } from "react";
import { IconButton, Stack, Typography } from "@mui/material";
import { PlayArrow, Pause, VolumeUp, VolumeOff } from "@mui/icons-material";
import type {Subtitle, SubtitleCue} from "./video-subtitle-viewer-types";
import {VideoControls, VideoWrapper} from "./video-subtitle-viewer-styles";
import {formatTime} from "./video-subtitle-viewer-utils";

interface VideoPlayerProps {
    mediaFile?: { url: string; type: string };
    currentSubtitle?: Subtitle | null;
    selectedSubtitle?: SubtitleCue | null;
    onTimeUpdate: (time: number) => void;
}

export const VideoPlayer = ({
                                mediaFile,
                                currentSubtitle,
                                selectedSubtitle,
                                onTimeUpdate
                            }: VideoPlayerProps) => {
    const videoRef = useRef<HTMLVideoElement>(null);
    const [isPlaying, setIsPlaying] = useState(false);
    const [isMuted, setIsMuted] = useState(false);
    const timeUpdateRef = useRef(false);

    const togglePlay = () => {
        if (videoRef.current) {
            isPlaying ? videoRef.current.pause() : videoRef.current.play();
            setIsPlaying(!isPlaying);
        }
    };

    const toggleMute = () => {
        if (videoRef.current) {
            videoRef.current.muted = !isMuted;
            setIsMuted(!isMuted);
        }
    };

    useEffect(() => {
        const handleTimeUpdate = () => {
            if (videoRef.current && !timeUpdateRef.current) {
                timeUpdateRef.current = true;
                onTimeUpdate(videoRef.current.currentTime);
                setTimeout(() => (timeUpdateRef.current = false), 50);
            }
        };

        const video = videoRef.current;
        video?.addEventListener("timeupdate", handleTimeUpdate);
        return () => video?.removeEventListener("timeupdate", handleTimeUpdate);
    }, [onTimeUpdate]);

    return (
        <VideoWrapper>
            <video
                ref={videoRef}
                style={{
                    width: '100%',
                    height: mediaFile?.type?.startsWith('audio') ? 'auto' : '100%'
                }}
                src={mediaFile?.url || "https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4"}
                onPlay={() => setIsPlaying(true)}
                onPause={() => setIsPlaying(false)}
            >
                {mediaFile?.type?.startsWith('audio') && (
                    <Typography variant="h6" sx={{ color: 'white', textAlign: 'center' }}>
                        Audio Visualization
                    </Typography>
                )}
                <track
                    kind="subtitles"
                    src={URL.createObjectURL(
                        new Blob([selectedSubtitle?.vttContent || ""], { type: "text/vtt" })
                    )}
                    srcLang="en"
                    label="English"
                    default
                />
            </video>

            <VideoControls>
                <Stack direction="row" alignItems="center" spacing={1}>
                    <IconButton onClick={togglePlay} sx={{ color: "common.white" }}>
                        {isPlaying ? <Pause /> : <PlayArrow />}
                    </IconButton>
                    <IconButton onClick={toggleMute} sx={{ color: "common.white" }}>
                        {isMuted ? <VolumeOff /> : <VolumeUp />}
                    </IconButton>
                    <Typography variant="body2" sx={{ color: "common.white", ml: 1 }}>
                        {currentSubtitle ? formatTime(currentSubtitle.start) : "00:00:00.000"}
                    </Typography>
                    <Typography variant="h6" sx={{ color: "rgba(255,255,255,0.6)" }}>
                        {currentSubtitle?.text.join(" ") || ""}
                    </Typography>
                </Stack>
            </VideoControls>
        </VideoWrapper>
    );
};
