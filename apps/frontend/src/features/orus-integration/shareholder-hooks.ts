/**
 * Comprehensive Shareholder hooks
 * Based on Orus useShareholder.ts and useShareholderAdmin.ts
 */
import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { User, Session } from "@supabase/supabase-js";
import { orusSupabase } from "./supabase-client";
import type {
  OrusShareholderParticipation,
  OrusShareholderComplianceInfo,
  OrusShareholderDocument,
  OrusShareholderDocumentRequest,
  OrusShareholderWithProfile,
  OrusUserRole,
} from "./types";
import { toast } from "sonner";

// Document types for compliance
export const DOCUMENT_TYPES = [
  { value: "id_card", label: "Carte d'identité" },
  { value: "passport", label: "Passeport" },
  { value: "proof_of_address", label: "Justificatif de domicile" },
  { value: "bank_details", label: "RIB" },
  { value: "tax_declaration", label: "Déclaration fiscale" },
  { value: "other", label: "Autre" },
] as const;

// =====================================================
// AUTH HOOK (with role management)
// =====================================================

export function useOrusAuthWithRole() {
  const [user, setUser] = useState<User | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);
  const [userRole, setUserRole] = useState<OrusUserRole>(null);

  useEffect(() => {
    const { data: { subscription } } = orusSupabase.auth.onAuthStateChange(
      (_event, session) => {
        setSession(session);
        setUser(session?.user ?? null);
        
        if (session?.user) {
          setTimeout(() => {
            fetchUserRole(session.user.id);
          }, 0);
        } else {
          setUserRole(null);
        }
      }
    );

    orusSupabase.auth.getSession().then(({ data: { session } }) => {
      setSession(session);
      setUser(session?.user ?? null);
      setLoading(false);
      
      if (session?.user) {
        fetchUserRole(session.user.id);
      }
    });

    return () => subscription.unsubscribe();
  }, []);

  const fetchUserRole = async (userId: string) => {
    try {
      // Cast to any to avoid type inference issues with dynamic tables
      const { data, error } = await (orusSupabase as any)
        .from('user_roles')
        .select('role')
        .eq('user_id', userId)
        .single();
      
      if (error) {
        console.error('Error fetching user role:', error);
        setUserRole(null);
        return;
      }
      
      setUserRole(data?.role as OrusUserRole);
    } catch (error) {
      console.error('Error in fetchUserRole:', error);
      setUserRole(null);
    }
  };

  return {
    user,
    session,
    loading,
    userRole,
    isAdmin: userRole === 'admin' || userRole === 'superadmin',
    isSuperadmin: userRole === 'superadmin',
  };
}

// =====================================================
// SHAREHOLDER HOOK (User-facing)
// =====================================================

export function useShareholder() {
  const { user } = useOrusAuthWithRole();
  const queryClient = useQueryClient();

  // Fetch user's participation
  const participationQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'participation', user?.id],
    queryFn: async () => {
      if (!user) return null;
      
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_participations')
        .select('*')
        .eq('user_id', user.id)
        .maybeSingle();

      if (error) {
        console.error('Error fetching participation:', error);
        throw error;
      }
      return data as OrusShareholderParticipation | null;
    },
    enabled: !!user,
  });

  // Fetch user's compliance info
  const complianceQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'compliance', user?.id],
    queryFn: async () => {
      if (!user) return null;
      
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_compliance_info')
        .select('*')
        .eq('user_id', user.id)
        .maybeSingle();

      if (error && error.code !== 'PGRST116') {
        console.error('Error fetching compliance info:', error);
        throw error;
      }
      return data as OrusShareholderComplianceInfo | null;
    },
    enabled: !!user,
  });

  // Fetch user's documents
  const documentsQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'documents', user?.id],
    queryFn: async () => {
      if (!user) return [];
      
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_documents')
        .select('*')
        .eq('user_id', user.id)
        .order('uploaded_at', { ascending: false });

      if (error) {
        console.error('Error fetching documents:', error);
        throw error;
      }
      return (data || []) as OrusShareholderDocument[];
    },
    enabled: !!user,
  });

  // Fetch document requests for user
  const documentRequestsQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'document_requests', user?.id],
    queryFn: async () => {
      if (!user) return [];
      
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_document_requests')
        .select('*')
        .eq('user_id', user.id)
        .order('created_at', { ascending: false });

      if (error) {
        console.error('Error fetching document requests:', error);
        throw error;
      }
      return (data || []) as OrusShareholderDocumentRequest[];
    },
    enabled: !!user,
  });

  // Fetch all participations (for pie chart)
  const allParticipationsQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'all_participations'],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_participations')
        .select('user_id, shares_count, total_value');

      if (error) {
        console.error('Error fetching all participations:', error);
        throw error;
      }
      return (data || []) as Array<{ user_id: string; shares_count: number; total_value: number }>;
    },
    enabled: !!user,
  });

  // Update compliance info mutation
  const updateComplianceMutation = useMutation({
    mutationFn: async (info: Partial<OrusShareholderComplianceInfo>) => {
      if (!user) throw new Error('Not authenticated');

      const { error } = await (orusSupabase as any)
        .from('shareholder_compliance_info')
        .upsert({
          user_id: user.id,
          ...info,
          updated_at: new Date().toISOString(),
        }, { onConflict: 'user_id' });

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success('Informations mises à jour');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'compliance'] });
    },
    onError: (error) => {
      console.error('Error updating compliance info:', error);
      toast.error('Erreur lors de la mise à jour des informations');
    },
  });

  // Upload document mutation
  const uploadDocumentMutation = useMutation({
    mutationFn: async ({ file, documentType }: { file: File; documentType: string }) => {
      if (!user) throw new Error('Not authenticated');

      const fileExt = file.name.split('.').pop();
      const fileName = `${user.id}/${Date.now()}-${documentType}.${fileExt}`;

      // Upload to storage
      const { error: uploadError } = await orusSupabase.storage
        .from('shareholder-documents')
        .upload(fileName, file);

      if (uploadError) throw uploadError;

      // Insert document record
      const { error: dbError } = await (orusSupabase as any)
        .from('shareholder_documents')
        .insert({
          user_id: user.id,
          document_type: documentType,
          document_name: file.name,
          file_path: fileName,
          file_size: file.size,
          mime_type: file.type,
        });

      if (dbError) throw dbError;
    },
    onSuccess: () => {
      toast.success('Document téléchargé avec succès');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'documents'] });
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'document_requests'] });
    },
    onError: (error) => {
      console.error('Error uploading document:', error);
      toast.error('Erreur lors du téléchargement du document');
    },
  });

  // Delete document mutation
  const deleteDocumentMutation = useMutation({
    mutationFn: async ({ documentId, filePath }: { documentId: string; filePath: string }) => {
      // Delete from storage
      const { error: storageError } = await orusSupabase.storage
        .from('shareholder-documents')
        .remove([filePath]);

      if (storageError) throw storageError;

      // Delete from database
      const { error: dbError } = await (orusSupabase as any)
        .from('shareholder_documents')
        .delete()
        .eq('id', documentId);

      if (dbError) throw dbError;
    },
    onSuccess: () => {
      toast.success('Document supprimé');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'documents'] });
    },
    onError: (error) => {
      console.error('Error deleting document:', error);
      toast.error('Erreur lors de la suppression du document');
    },
  });

  // Get signed URL for document
  const getDocumentUrl = async (filePath: string): Promise<string | null> => {
    const { data, error } = await orusSupabase.storage
      .from('shareholder-documents')
      .createSignedUrl(filePath, 60 * 60); // 1 hour

    if (error) {
      console.error('Error getting document URL:', error);
      return null;
    }
    return data.signedUrl;
  };

  return {
    user,
    participation: participationQuery.data,
    complianceInfo: complianceQuery.data,
    documents: documentsQuery.data || [],
    documentRequests: documentRequestsQuery.data || [],
    allParticipations: allParticipationsQuery.data || [],
    loading: participationQuery.isLoading || complianceQuery.isLoading,
    error: participationQuery.error || complianceQuery.error,
    updateComplianceInfo: (data: Partial<OrusShareholderComplianceInfo>) =>
      updateComplianceMutation.mutate(data),
    uploadDocument: (file: File, documentType: string) =>
      uploadDocumentMutation.mutate({ file, documentType }),
    deleteDocument: (documentId: string, filePath: string) =>
      deleteDocumentMutation.mutate({ documentId, filePath }),
    getDocumentUrl,
  };
}

// =====================================================
// SHAREHOLDER ADMIN HOOK
// =====================================================

export function useShareholderAdmin() {
  const { user, isAdmin, isSuperadmin } = useOrusAuthWithRole();
  const queryClient = useQueryClient();

  // Fetch all shareholders with their profiles
  const shareholdersQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'admin', 'all'],
    queryFn: async () => {
      // Get all participations (no join - FK points to auth.users, not profiles)
      const { data: participations, error: partError } = await (orusSupabase as any)
        .from('shareholder_participations')
        .select('*')
        .order('shares_count', { ascending: false });

      if (partError) {
        console.error('Error fetching participations:', partError);
        throw partError;
      }

      if (!participations || participations.length === 0) {
        return [];
      }

      // Get unique user_ids
      const userIds = [...new Set(participations.map((p: any) => p.user_id))];

      // Fetch profiles for these users
      const { data: profiles, error: profilesError } = await (orusSupabase as any)
        .from('profiles')
        .select('id, email, full_name')
        .in('id', userIds);

      if (profilesError) {
        console.error('Error fetching profiles:', profilesError);
        // Continue without profiles rather than failing
      }

      // Create a map of user_id -> profile
      const profilesMap: Record<string, any> = {};
      (profiles || []).forEach((p: any) => {
        profilesMap[p.id] = p;
      });

      // Map to OrusShareholderWithProfile format
      const shareholders: OrusShareholderWithProfile[] = participations.map((p: any) => ({
        participation: {
          id: p.id,
          user_id: p.user_id,
          shares_count: p.shares_count,
          share_value: p.share_value,
          total_value: p.total_value,
          currency: p.currency,
          investment_date: p.investment_date,
          last_valuation_date: p.last_valuation_date,
          notes: p.notes,
          created_at: p.created_at,
          updated_at: p.updated_at,
        },
        profile: profilesMap[p.user_id] || null,
      }));

      return shareholders;
    },
    enabled: isAdmin,
  });

  // Fetch compliance info for all shareholders (admin only)
  const complianceQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'admin', 'compliance'],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_compliance_info')
        .select('*');

      if (error) {
        console.error('Error fetching compliance info:', error);
        throw error;
      }

      // Create a map of user_id -> compliance info
      const complianceMap: Record<string, OrusShareholderComplianceInfo> = {};
      (data || []).forEach((c: OrusShareholderComplianceInfo) => {
        complianceMap[c.user_id] = c;
      });
      return complianceMap;
    },
    enabled: isSuperadmin,
  });

  // Fetch documents for all shareholders (admin only)
  const documentsQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'admin', 'documents'],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_documents')
        .select('*')
        .order('uploaded_at', { ascending: false });

      if (error) {
        console.error('Error fetching documents:', error);
        throw error;
      }

      // Create a map of user_id -> documents
      const documentsMap: Record<string, OrusShareholderDocument[]> = {};
      (data || []).forEach((d: OrusShareholderDocument) => {
        if (!documentsMap[d.user_id]) {
          documentsMap[d.user_id] = [];
        }
        documentsMap[d.user_id].push(d);
      });
      return documentsMap;
    },
    enabled: isAdmin,
  });

  // Fetch all document requests
  const documentRequestsQuery = useQuery({
    queryKey: ['orus', 'shareholder', 'admin', 'document_requests'],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from('shareholder_document_requests')
        .select('*')
        .order('created_at', { ascending: false });

      if (error) {
        console.error('Error fetching document requests:', error);
        throw error;
      }

      // Map by user_id
      const requestsMap: Record<string, OrusShareholderDocumentRequest[]> = {};
      (data || []).forEach((r: OrusShareholderDocumentRequest) => {
        if (!requestsMap[r.user_id]) {
          requestsMap[r.user_id] = [];
        }
        requestsMap[r.user_id].push(r);
      });
      return requestsMap;
    },
    enabled: isAdmin,
  });

  // Combine all data into shareholders with full info
  const shareholders: OrusShareholderWithProfile[] = (shareholdersQuery.data || []).map((s) => ({
    ...s,
    complianceInfo: complianceQuery.data?.[s.participation.user_id] || null,
    documents: documentsQuery.data?.[s.participation.user_id] || [],
    documentRequests: documentRequestsQuery.data?.[s.participation.user_id] || [],
    pendingRequests:
      documentRequestsQuery.data?.[s.participation.user_id]?.filter(
        (r: OrusShareholderDocumentRequest) => r.status === 'pending'
      ).length || 0,
  }));

  // Update participation mutation
  const updateParticipationMutation = useMutation({
    mutationFn: async (data: Partial<OrusShareholderParticipation> & { id: string }) => {
      const { error } = await (orusSupabase as any)
        .from('shareholder_participations')
        .update({
          ...data,
          total_value: data.shares_count && data.share_value
            ? data.shares_count * data.share_value
            : undefined,
          updated_at: new Date().toISOString(),
        })
        .eq('id', data.id);

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success('Participation mise à jour');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'admin'] });
    },
    onError: (error) => {
      console.error('Error updating participation:', error);
      toast.error('Erreur lors de la mise à jour');
    },
  });

  // Create participation mutation
  const createParticipationMutation = useMutation({
    mutationFn: async (data: Omit<OrusShareholderParticipation, 'id' | 'total_value' | 'created_at' | 'updated_at'>) => {
      const { error } = await (orusSupabase as any)
        .from('shareholder_participations')
        .insert({
          ...data,
          total_value: data.shares_count * data.share_value,
        });

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success('Actionnaire ajouté');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'admin'] });
    },
    onError: (error) => {
      console.error('Error creating participation:', error);
      toast.error('Erreur lors de la création');
    },
  });

  // Request document mutation
  const requestDocumentMutation = useMutation({
    mutationFn: async (data: { userId: string; documentType: string; description?: string; dueDate?: string }) => {
      if (!user) throw new Error('Not authenticated');

      const { error } = await (orusSupabase as any)
        .from('shareholder_document_requests')
        .insert({
          user_id: data.userId,
          requested_by: user.id,
          document_type: data.documentType,
          description: data.description,
          due_date: data.dueDate,
          status: 'pending',
        });

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success('Demande de document envoyée');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'admin', 'document_requests'] });
    },
    onError: (error) => {
      console.error('Error requesting document:', error);
      toast.error('Erreur lors de la demande');
    },
  });

  // Validate/reject document request mutation
  const validateDocumentRequestMutation = useMutation({
    mutationFn: async ({ requestId, status, documentId }: { requestId: string; status: 'fulfilled' | 'rejected'; documentId?: string }) => {
      const { error } = await (orusSupabase as any)
        .from('shareholder_document_requests')
        .update({
          status,
          fulfilled_at: new Date().toISOString(),
          fulfilled_document_id: documentId,
        })
        .eq('id', requestId);

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success('Statut mis à jour');
      queryClient.invalidateQueries({ queryKey: ['orus', 'shareholder', 'admin', 'document_requests'] });
    },
    onError: (error) => {
      console.error('Error updating document request:', error);
      toast.error('Erreur lors de la mise à jour');
    },
  });

  // Get signed URL for document download
  const getDocumentUrl = async (filePath: string): Promise<string | null> => {
    const { data, error } = await orusSupabase.storage
      .from('shareholder-documents')
      .createSignedUrl(filePath, 60 * 60); // 1 hour

    if (error) {
      console.error('Error getting document URL:', error);
      return null;
    }
    return data.signedUrl;
  };

  return {
    shareholders,
    loading: shareholdersQuery.isLoading,
    error: shareholdersQuery.error,
    isAdmin,
    isSuperadmin,
    user,
    updateParticipation: (data: Partial<OrusShareholderParticipation> & { id: string }) =>
      updateParticipationMutation.mutate(data),
    createParticipation: (data: Omit<OrusShareholderParticipation, 'id' | 'total_value' | 'created_at' | 'updated_at'>) =>
      createParticipationMutation.mutate(data),
    requestDocument: (data: { userId: string; documentType: string; description?: string; dueDate?: string }) =>
      requestDocumentMutation.mutate(data),
    validateDocumentRequest: (data: { requestId: string; status: 'fulfilled' | 'rejected'; documentId?: string }) =>
      validateDocumentRequestMutation.mutate(data),
    getDocumentUrl,
  };
}
