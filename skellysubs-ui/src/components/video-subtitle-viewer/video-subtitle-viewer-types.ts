import type {SubtitleVariant} from "../../store/slices/available-subtitles/available-subtitles-types";

export interface Subtitle {
    start: number;
    end: number;
    text: string[];
}

export interface SubtitleCue {
    id: string;
    name: string;
    variant: SubtitleVariant;
    format: string;
    language: string;
    content: string;
}
