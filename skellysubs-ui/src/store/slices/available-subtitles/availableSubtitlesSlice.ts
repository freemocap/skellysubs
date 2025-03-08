// subtitleOptionsSlice.ts
import type { PayloadAction } from "@reduxjs/toolkit";
import { createSlice } from "@reduxjs/toolkit";
import type { RootState } from "../../AppStateStore";
import type {SubtitleFormat, SubtitleVariant} from "./available-subtitles-types";

export interface AvailableSubtitles {
    id: string;
    name: string;
    variant: SubtitleVariant;
    format: SubtitleFormat;
    language: string;
    content: string;
}

interface AvailableSubtitlesState {
    availableSubtitles: AvailableSubtitles[];
    selectedId: string | null;
}

const initialState: AvailableSubtitlesState = {
    availableSubtitles: [],
    selectedId: null,
};
export interface SubtitleQuery {
    language: string;
    variant: SubtitleVariant;
    format: "srt" | "md";
}
export const availableSubtitlesSlice = createSlice({
    name: "availableSubtitles",
    initialState,
    reducers: {
        addAvailableSubtitles: (state, action: PayloadAction<AvailableSubtitles>) => {
            const existing = state.availableSubtitles.find(s => s.id === action.payload.id);
            if (!existing) {
                state.availableSubtitles.push(action.payload);
                if (!state.selectedId) state.selectedId = action.payload.id;
            }
        },
        updateAvailableSubtitles: (
            state,
            action: PayloadAction<{ id: string; content: string }>
        ) => {
            const subtitle = state.availableSubtitles.find(s => s.id === action.payload.id);
            if (subtitle) subtitle.content = action.payload.content;
        },
        selectAvailableSubtitles: (state, action: PayloadAction<string>) => {
            if (state.availableSubtitles.some(s => s.id === action.payload)) {
                state.selectedId = action.payload;
            }
        },
    },
});

export const { addAvailableSubtitles, updateAvailableSubtitles, selectAvailableSubtitles } =
    availableSubtitlesSlice.actions;

export const selectSubtitles = (app_state: RootState) =>
    app_state.subtitles.availableSubtitles;

export const selectSelectedSubtitle = (app_state: RootState) =>
    app_state.subtitles.availableSubtitles.find(
        s => s.id === app_state.subtitles.selectedId
    );

export const selectSubtitlesByLanguage = (app_state: RootState, language: string) =>
    app_state.subtitles.availableSubtitles.filter(s => s.language === language);
